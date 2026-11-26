//! PR #17.A: bounded, single-owner connection workers.
//! No caller integration in this phase. Existing authorization and codecs remain caller responsibilities.
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, watch, Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;

use crate::modbus::common::SerialMode;
use super::serial::{SerialPortConfig, SerialTransport};
use super::tcp::{TcpConfig, TcpFraming, TcpTransport};
use super::udp::UdpTransport;
use super::timing::{micros, TransactionTiming};
use super::{Transport, TransportError, TransportResult};

pub const QUEUE_CAPACITY: usize = 128;
const INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const MAX_BACKOFF: Duration = Duration::from_secs(5);
const MAX_OPERATION: Duration = Duration::from_secs(60);
const MAX_OPEN: Duration = Duration::from_secs(20);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportConfig {
    Serial { config: SerialPortConfig, mode: SerialMode },
    Tcp(TcpConfig),
    Udp { host: String, port: u16, mbap: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionMode { Persistent, OnDemand }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPolicy {
    pub mode: ConnectionMode,
    pub auto_reconnect: bool,
}
impl Default for ConnectionPolicy {
    fn default() -> Self {
        Self { mode: ConnectionMode::Persistent, auto_reconnect: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionDefinition {
    pub id: String,
    pub name: String,
    pub transport: TransportConfig,
    pub policy: ConnectionPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus { Disconnected, Connecting, Connected, Faulted, RetryWait, Disconnecting }

#[derive(Debug, Clone)]
pub struct ConnectionRuntimeState {
    pub connection_id: String,
    pub status: ConnectionStatus,
    pub queue_depth: usize,
    pub active_requests: usize,
    pub reconnect_count: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum Priority { High, Normal, Low }
impl Priority {
    fn index(self) -> usize { match self { Self::High => 0, Self::Normal => 1, Self::Low => 2 } }
}

pub struct TransactionRequest {
    pub adu: Vec<u8>,
    pub response_timeout: Duration,
    pub expected_hint: usize,
    pub priority: Priority,
}

pub struct TransactionOutcome {
    pub result: TransportResult<Vec<u8>>,
    pub timing: TransactionTiming,
    pub queue_wait_us: u64,
}

fn invalid(message: &str) -> TransportError { TransportError::Other(message.into()) }

fn network_key(host: &str, port: u16, kind: &str) -> TransportResult<String> {
    let host = host.trim();
    if host.is_empty() || port == 0 { return Err(invalid("invalid network endpoint")); }
    let ip_host = host.strip_prefix('[').and_then(|v|v.strip_suffix(']')).unwrap_or(host);
    let canonical = ip_host.parse::<std::net::IpAddr>().map(|ip| ip.to_string())
        .unwrap_or_else(|_| host.trim_end_matches('.').to_ascii_lowercase());
    if canonical.is_empty() { return Err(invalid("invalid network endpoint")); }
    Ok(format!("{kind}:{canonical}:{port}"))
}

fn serial_key(port: &str) -> TransportResult<String> {
    let port = port.trim();
    if port.is_empty() { return Err(invalid("empty serial port")); }
    #[cfg(windows)]
    {
        let normalized = port.to_ascii_uppercase();
        let name = normalized.strip_prefix(r"\\.\").unwrap_or(&normalized);
        if let Some(number) = name.strip_prefix("COM").and_then(|v|v.parse::<u32>().ok()) {
            return Ok(format!("serial:COM{number}"));
        }
        Ok(format!("serial:{name}"))
    }
    #[cfg(not(windows))]
    {
        let path = std::fs::canonicalize(port).unwrap_or_else(|_|std::path::PathBuf::from(port));
        Ok(format!("serial:{}",path.to_string_lossy()))
    }
}

impl ConnectionDefinition {
    fn resource_key(&self) -> TransportResult<String> {
        if self.id.trim().is_empty() || self.name.trim().is_empty() {
            return Err(invalid("connection id and name required"));
        }
        match &self.transport {
            TransportConfig::Serial { config, .. } => {
                if config.baud_rate == 0 || config.frame_gap > MAX_OPERATION {
                    return Err(invalid("invalid serial timing"));
                }
                serial_key(&config.port_name)
            }
            TransportConfig::Tcp(config) => {
                if config.connect_timeout.is_zero() || config.connect_timeout > MAX_OPEN {
                    return Err(invalid("invalid TCP connect timeout"));
                }
                if config.tls.is_some() && config.framing != TcpFraming::Mbap {
                    return Err(invalid("TLS requires MBAP framing"));
                }
                network_key(&config.host, config.port, "tcp")
            }
            TransportConfig::Udp {host,port,..} => {
                // Existing UDP implementation accepts numeric SocketAddr only.
                format!("{host}:{port}").parse::<std::net::SocketAddr>()
                    .map_err(|_|invalid("UDP requires a numeric socket address"))?;
                network_key(host,*port,"udp")
            }
        }
    }
}

#[async_trait]
pub trait TransportFactory: Send + Sync {
    async fn open(&self, config: &TransportConfig) -> TransportResult<Box<dyn Transport>>;
}

struct NativeFactory;
#[async_trait]
impl TransportFactory for NativeFactory {
    async fn open(&self, config: &TransportConfig) -> TransportResult<Box<dyn Transport>> {
        match config {
            TransportConfig::Serial {config,mode} => Ok(Box::new(SerialTransport::open(config.clone(),*mode).await?)),
            TransportConfig::Tcp(config) => Ok(Box::new(TcpTransport::connect(config.clone()).await?)),
            TransportConfig::Udp {host,port,..} => Ok(Box::new(UdpTransport::connect(host,*port).await?)),
        }
    }
}

struct Job {
    request: TransactionRequest,
    queued: Instant,
    reply: oneshot::Sender<TransactionOutcome>,
    _slot: OwnedSemaphorePermit,
}

// Weighted round robin; FIFO within each class. Empty classes are skipped.
// At sustained load all three classes receive service in each six dispatches.
const SCHEDULE: [usize; 6] = [0,0,0,1,1,2];
struct FairQueue<T> {
    queues: [VecDeque<T>;3],
    cursor: usize,
}
impl<T> Default for FairQueue<T> {
    fn default()->Self {Self{queues:std::array::from_fn(|_|VecDeque::new()),cursor:0}}
}
impl<T> FairQueue<T> {
    fn push(&mut self,priority:Priority,item:T){self.queues[priority.index()].push_back(item);}
    fn pop(&mut self)->Option<T>{
        for _ in 0..SCHEDULE.len(){
            let i=SCHEDULE[self.cursor];
            self.cursor=(self.cursor+1)%SCHEDULE.len();
            if let Some(item)=self.queues[i].pop_front(){return Some(item);}
        }
        None
    }
}

struct Entry {
    definition: ConnectionDefinition,
    resource: String,
    tx: mpsc::Sender<Job>,
    slots: Arc<Semaphore>,
    stop: watch::Sender<bool>,
    state: Arc<parking_lot::Mutex<ConnectionRuntimeState>>,
    task: Mutex<Option<JoinHandle<()>>>,
}

pub struct ConnectionManager {
    entries: parking_lot::Mutex<HashMap<String,Arc<Entry>>>,
    factory: Arc<dyn TransportFactory>,
}
impl Default for ConnectionManager {fn default()->Self{Self::new()}}
impl ConnectionManager {
    pub fn new()->Self{Self::with_factory(Arc::new(NativeFactory))}
    pub fn with_factory(factory:Arc<dyn TransportFactory>)->Self{
        Self{entries:parking_lot::Mutex::new(HashMap::new()),factory}
    }

    /// Register one lazy worker. Opening occurs on its first request.
    /// The registry retains ownership until disconnect has joined the worker.
    pub fn register(&self,definition:ConnectionDefinition)->TransportResult<()> {
        let runtime=tokio::runtime::Handle::try_current().map_err(|_|invalid("connection registration requires Tokio runtime"))?;
        let resource=definition.resource_key()?;
        let mut entries=self.entries.lock();
        if entries.contains_key(&definition.id) {return Err(invalid("connection id already registered"));}
        if entries.values().any(|e|e.resource==resource) {return Err(invalid("physical resource already owned"));}
        let (tx,rx)=mpsc::channel(QUEUE_CAPACITY);
        let (stop,stop_rx)=watch::channel(false);
        let state=Arc::new(parking_lot::Mutex::new(ConnectionRuntimeState{
            connection_id:definition.id.clone(),status:ConnectionStatus::Disconnected,
            queue_depth:0,active_requests:0,reconnect_count:0,last_error:None,
        }));
        let slots=Arc::new(Semaphore::new(QUEUE_CAPACITY));
        let task=runtime.spawn(worker(definition.clone(),self.factory.clone(),state.clone(),rx,stop_rx));
        entries.insert(definition.id.clone(),Arc::new(Entry{definition,resource,tx,slots,stop,state,task:Mutex::new(Some(task))}));
        Ok(())
    }

    fn entry(&self,id:&str)->TransportResult<Arc<Entry>>{
        self.entries.lock().get(id).cloned().ok_or_else(||invalid("unknown connection"))
    }

    pub fn definitions(&self)->Vec<ConnectionDefinition>{
        let mut rows:Vec<_>=self.entries.lock().values().map(|e|e.definition.clone()).collect();
        rows.sort_by(|a,b|a.id.cmp(&b.id));rows
    }

    pub fn snapshot(&self)->Vec<ConnectionRuntimeState>{
        let entries=self.entries.lock();
        let mut rows:Vec<_>=entries.values().map(|e|{
            let mut s=e.state.lock().clone();
            s.queue_depth=QUEUE_CAPACITY-e.slots.available_permits();
            s
        }).collect();
        rows.sort_by(|a,b|a.connection_id.cmp(&b.connection_id));rows
    }

    /// Immediate backpressure: at most 128 waiting jobs plus one active job.
    /// Dropping the caller future cancels a queued job; an active operation is
    /// allowed to finish within its bound, never replayed.
    pub async fn transact(&self,id:&str,request:TransactionRequest)->TransportResult<TransactionOutcome>{
        if request.adu.is_empty() || request.adu.len()>513 || request.response_timeout.is_zero()
            || request.response_timeout>MAX_OPERATION || request.expected_hint>1024 {
            return Err(invalid("invalid transaction bounds"));
        }
        let entry=self.entry(id)?;
        if *entry.stop.borrow(){return Err(TransportError::Cancelled);}
        let slot=entry.slots.clone().try_acquire_owned().map_err(|_|invalid("connection queue full"))?;
        let (reply,rx)=oneshot::channel();
        entry.tx.try_send(Job{request,queued:Instant::now(),reply,_slot:slot})
            .map_err(|e|match e {mpsc::error::TrySendError::Full(_)=>invalid("connection queue full"),mpsc::error::TrySendError::Closed(_)=>TransportError::Cancelled})?;
        rx.await.map_err(|_|TransportError::Cancelled)
    }

    /// Stop outside the queue. Keep the lease until the worker has exited,
    /// including when a caller cancels its disconnect future.
    pub async fn disconnect(&self,id:&str)->TransportResult<()>{
        let entry=self.entry(id)?;
        entry.stop.send_replace(true);
        let mut task=entry.task.lock().await;
        let joined=if let Some(handle)=task.as_mut(){handle.await.map_err(|e|TransportError::Other(e.to_string()))}else{Ok(())};
        task.take();
        let mut entries=self.entries.lock();
        if entries.get(id).is_some_and(|e|Arc::ptr_eq(e,&entry)){entries.remove(id);}
        joined
    }
}

impl Drop for ConnectionManager {
    fn drop(&mut self){
        for e in self.entries.get_mut().values(){e.stop.send_replace(true);}
    }
}

fn set_status(state:&parking_lot::Mutex<ConnectionRuntimeState>,status:ConnectionStatus){
    state.lock().status=status;
}
async fn close(transport:&mut Option<Box<dyn Transport>>){
    if let Some(mut t)=transport.take(){let _=tokio::time::timeout(CLOSE_TIMEOUT,t.close()).await;}
}

fn cancelled(job:Job){
    let _=job.reply.send(TransactionOutcome{result:Err(TransportError::Cancelled),timing:TransactionTiming::default(),queue_wait_us:micros(job.queued)});
}

async fn worker(
    definition:ConnectionDefinition,factory:Arc<dyn TransportFactory>,
    state:Arc<parking_lot::Mutex<ConnectionRuntimeState>>,mut rx:mpsc::Receiver<Job>,mut stop:watch::Receiver<bool>,
){
    let mut transport:Option<Box<dyn Transport>>=None;
    let mut pending=FairQueue::default();
    let mut backoff=INITIAL_BACKOFF;
    let mut retry_after:Option<tokio::time::Instant>=None;
    let mut faulted=false;
    let mut opened_before=false;
    loop{
        if *stop.borrow(){break;}
        while let Ok(job)=rx.try_recv(){pending.push(job.request.priority,job);}
        let job=match pending.pop(){
            Some(job)=>job,
            None=>tokio::select!{biased;
                _=stop.changed()=>break,
                next=rx.recv()=>match next{Some(job)=>{pending.push(job.request.priority,job);continue},None=>break}
            }
        };
        if *stop.borrow(){cancelled(job);break;}
        let Job{request,queued,mut reply,_slot}=job;
        if reply.is_closed(){continue;}
        drop(_slot);
        let queue_wait_us=micros(queued);
        state.lock().active_requests=1;
        let mut timing=TransactionTiming::default();
        let result=async{
            if transport.is_none(){
                if faulted&&!definition.policy.auto_reconnect {
                    return Err(invalid("connection faulted; disconnect and register again"));
                }
                if let Some(until)=retry_after{
                    set_status(&state,ConnectionStatus::RetryWait);
                    tokio::select!{biased;
                        _=stop.changed()=>return Err(TransportError::Cancelled),
                        _=reply.closed()=>return Err(TransportError::Cancelled),
                        _=tokio::time::sleep_until(until)=>{}
                    }
                }
                if *stop.borrow() || reply.is_closed(){return Err(TransportError::Cancelled);}
                set_status(&state,ConnectionStatus::Connecting);
                let start=Instant::now();
                let opened=tokio::time::timeout(MAX_OPEN,factory.open(&definition.transport)).await;
                timing.connect_us=Some(micros(start));
                transport=Some(match opened{
                    Ok(Ok(t))=>t,
                    Ok(Err(e))=>{timing.failure_stage=Some("connect".into());return Err(e)},
                    Err(_)=>{timing.failure_stage=Some("connect".into());return Err(TransportError::Timeout(MAX_OPEN))}
                });
                let mut s=state.lock();
                if opened_before&&faulted{s.reconnect_count+=1;}
                opened_before=true;
                s.status=ConnectionStatus::Connected;
                s.last_error=None;
            }
            if *stop.borrow() || reply.is_closed(){return Err(TransportError::Cancelled);}
            let t=transport.as_mut().ok_or(TransportError::Closed)?;
            // Bound reset/TX as well as RX; the existing response timeout still applies.
            let budget=request.response_timeout.saturating_add(Duration::from_secs(2));
            match tokio::time::timeout(budget,t.transact_measured(&request.adu,request.response_timeout,request.expected_hint,&mut timing)).await{
                Ok(result)=>result,
                Err(_)=>{timing.failure_stage=Some("operation_deadline".into());Err(TransportError::Timeout(budget))}
            }
        }.await;
        match &result{
            Ok(_)=>{
                faulted=false;retry_after=None;backoff=INITIAL_BACKOFF;
                if definition.policy.mode==ConnectionMode::OnDemand{
                    close(&mut transport).await;
                    set_status(&state,ConnectionStatus::Disconnected);
                }
            }
            Err(TransportError::Cancelled)=>{},
            Err(e)=>{
                close(&mut transport).await;
                faulted=true;
                retry_after=Some(tokio::time::Instant::now()+backoff);
                backoff=(backoff*2).min(MAX_BACKOFF);
                let mut s=state.lock();
                s.status=ConnectionStatus::Faulted;s.last_error=Some(e.to_string());
            }
        }
        state.lock().active_requests=0;
        let _=reply.send(TransactionOutcome{result,timing,queue_wait_us});
    }
    set_status(&state,ConnectionStatus::Disconnecting);
    rx.close();
    while let Some(job)=pending.pop(){cancelled(job);}
    while let Some(job)=rx.recv().await{cancelled(job);}
    close(&mut transport).await;
    set_status(&state,ConnectionStatus::Disconnected);
}

#[cfg(test)]
mod tests;

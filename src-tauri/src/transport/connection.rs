//! PR #17.A: bounded, single-owner connection workers.
//! Shared resource ownership. Existing authorization and codecs remain caller responsibilities.
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
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

mod lifecycle;
pub use lifecycle::{ConnectionLease, ExclusiveSerialSession, SerialSessionSnapshot, IDLE_TCP, IDLE_SERIAL, IDLE_CACHE_LIMIT};
use lifecycle::{LeaseState, RecoveryBook};

pub const QUEUE_CAPACITY: usize = 128;
pub const MANUAL_RESERVE: usize = 8;
const NORMAL_CAPACITY: usize = QUEUE_CAPACITY - MANUAL_RESERVE;
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
    /// None: derive from timeout and framing. Some: minimum additional quiet time.
    #[serde(default)]
    pub recovery_ms: Option<u32>,
}
impl Default for ConnectionPolicy {
    fn default() -> Self {
        Self { mode: ConnectionMode::Persistent, auto_reconnect: true, recovery_ms: None }
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
    pub queue_rejected: u64,
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
    /// Complete ADU rejected by the validator; retain it for traffic diagnostics.
    pub rejected_response: Option<Vec<u8>>,
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
    pub fn resource_key(&self) -> TransportResult<String> {
        if self.policy.recovery_ms.is_some_and(|ms|ms>60_000){return Err(invalid("recovery pause exceeds 60 seconds"));}
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

/// One tool operation (scan, monitor session or manual command).
/// Stop closes admission atomically with dispatch, then waits only for its own active jobs.
pub struct OperationScope {
    cancelled: watch::Sender<bool>,
    active: watch::Sender<usize>,
    gate: parking_lot::Mutex<()>,
}
impl Default for OperationScope {
    fn default()->Self{Self{cancelled:watch::channel(false).0,active:watch::channel(0).0,gate:parking_lot::Mutex::new(())}}
}
impl OperationScope {
    pub fn is_cancelled(&self)->bool{*self.cancelled.borrow()}
    pub fn cancel(&self){let _guard=self.gate.lock();self.cancelled.send_replace(true);}
    pub async fn stop(&self){
        self.cancel();
        let mut active=self.active.subscribe();
        while *active.borrow_and_update()!=0 {if active.changed().await.is_err(){break;}}
    }
    fn begin(self:&Arc<Self>,started:&AtomicBool)->Option<ActiveScope>{
        let _guard=self.gate.lock();
        if self.is_cancelled(){return None;}
        started.store(true,Ordering::Release);
        let count=*self.active.borrow();
        self.active.send_replace(count+1);
        Some(ActiveScope(self.clone()))
    }
}
struct ActiveScope(Arc<OperationScope>);
impl Drop for ActiveScope{
    fn drop(&mut self){
        let _guard=self.0.gate.lock();
        let count=*self.0.active.borrow();
        self.0.active.send_replace(count.saturating_sub(1));
    }
}
pub type ResponseValidator = Box<dyn Fn(&[u8])->TransportResult<()> + Send + Sync>;

struct Job {
    request: TransactionRequest,
    queued: Instant,
    reply: oneshot::Sender<TransactionOutcome>,
    _slot: OwnedSemaphorePermit,
    _regular: Option<OwnedSemaphorePermit>,
    scope: Arc<OperationScope>,
    started: Arc<AtomicBool>,
    validator: Option<ResponseValidator>,
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
    regular: Arc<Semaphore>,
    stop: watch::Sender<bool>,
    state: Arc<parking_lot::Mutex<ConnectionRuntimeState>>,
    task: Mutex<Option<JoinHandle<()>>>,
    lifecycle: parking_lot::Mutex<LeaseState>,
}

pub struct ConnectionManager {
    entries: parking_lot::Mutex<HashMap<String,Arc<Entry>>>,
    factory: Arc<dyn TransportFactory>,
    exclusive: Arc<parking_lot::Mutex<HashSet<String>>>,
    recovery: Arc<RecoveryBook>,
    reaper_started: AtomicBool,
}
impl Default for ConnectionManager {fn default()->Self{Self::new()}}
impl ConnectionManager {
    pub fn new()->Self{Self::with_factory(Arc::new(NativeFactory))}
    pub fn with_factory(factory:Arc<dyn TransportFactory>)->Self{
        Self{entries:parking_lot::Mutex::new(HashMap::new()),factory,exclusive:Arc::new(parking_lot::Mutex::new(HashSet::new())),recovery:Arc::new(RecoveryBook::default()),reaper_started:AtomicBool::new(false)}
    }

    /// Register one lazy worker. Opening occurs on its first request.
    /// The registry retains ownership until disconnect has joined the worker.
    pub fn register(&self,definition:ConnectionDefinition)->TransportResult<()> {
        let runtime=tokio::runtime::Handle::try_current().map_err(|_|invalid("connection registration requires Tokio runtime"))?;
        let resource=definition.resource_key()?;
        let mut entries=self.entries.lock();
        if entries.contains_key(&definition.id) {return Err(invalid("connection id already registered"));}
        if entries.values().any(|e|e.resource==resource) || self.exclusive.lock().contains(&resource) {return Err(invalid("physical resource already owned"));}
        self.insert_entry(&mut entries,definition,resource,LeaseState::explicit(),&runtime);
        Ok(())
    }

    fn insert_entry(&self,entries:&mut HashMap<String,Arc<Entry>>,definition:ConnectionDefinition,resource:String,lifecycle:LeaseState,runtime:&tokio::runtime::Handle)->Arc<Entry>{
        let (tx,rx)=mpsc::channel(QUEUE_CAPACITY);
        let (stop,stop_rx)=watch::channel(false);
        let state=Arc::new(parking_lot::Mutex::new(ConnectionRuntimeState{
            connection_id:definition.id.clone(),status:ConnectionStatus::Disconnected,
            queue_depth:0,active_requests:0,reconnect_count:0,queue_rejected:0,last_error:None,
        }));
        let slots=Arc::new(Semaphore::new(QUEUE_CAPACITY));
        let regular=Arc::new(Semaphore::new(NORMAL_CAPACITY));
        let task=runtime.spawn(worker(definition.clone(),self.factory.clone(),state.clone(),rx,stop_rx,resource.clone(),self.recovery.clone()));
        let id=definition.id.clone();
        let entry=Arc::new(Entry{definition,resource,tx,slots,regular,stop,state,task:Mutex::new(Some(task)),lifecycle:parking_lot::Mutex::new(lifecycle)});
        entries.insert(id,entry.clone());
        entry
    }

    /// Exclusive lease for discovery which must change serial framing.
    /// No registered worker can own this port until the lease is dropped.
    pub fn lease_serial(&self,port:&str)->TransportResult<SerialLease>{
        self.lease_serials([port])
    }

    /// Atomically claim all ports in a legacy operation, including a serial
    /// sweep. The owned guard can be moved into a spawned task.
    pub fn lease_serials<'a>(&self,ports:impl IntoIterator<Item=&'a str>)->TransportResult<SerialLease>{
        let resources:HashSet<String>=ports.into_iter().map(serial_key).collect::<TransportResult<_>>()?;
        let entries=self.entries.lock();
        let mut exclusive=self.exclusive.lock();
        if resources.iter().any(|r|entries.values().any(|e|&e.resource==r)||exclusive.contains(r)){
            return Err(invalid("physical serial resource already owned"));
        }
        exclusive.extend(resources.iter().cloned());
        Ok(SerialLease{exclusive:self.exclusive.clone(),resources})
    }

    /// A bounded, operation-local worker. Used while the persistent connection
    /// editor and shared session lifecycle are not yet integrated.
    /// Call close().await for deterministic cleanup; Drop is a cancellation fallback.
    pub fn temporary(self:&Arc<Self>,definition:ConnectionDefinition)->TransportResult<TemporaryConnection>{
        let id=definition.id.clone();
        self.register(definition)?;
        Ok(TemporaryConnection{manager:self.clone(),id,scope:Arc::new(OperationScope::default()),closed:false})
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
        self.transact_scoped(id,request,Arc::new(OperationScope::default()),None).await
    }

    /// Validator executes inside the worker before another transaction can start.
    pub async fn transact_scoped(&self,id:&str,request:TransactionRequest,scope:Arc<OperationScope>,validator:Option<ResponseValidator>)->TransportResult<TransactionOutcome>{
        let entry=self.entry(id)?;
        if entry.lifecycle.lock().automatic{return Err(invalid("automatic connection requires an acquired lease"));}
        self.transact_entry(entry,request,scope,validator).await
    }
    async fn transact_entry(&self,entry:Arc<Entry>,request:TransactionRequest,scope:Arc<OperationScope>,validator:Option<ResponseValidator>)->TransportResult<TransactionOutcome>{
        if request.adu.is_empty() || request.adu.len()>513 || request.response_timeout.is_zero()
            || request.response_timeout>MAX_OPERATION || request.expected_hint>1024 {
            return Err(invalid("invalid transaction bounds"));
        }
        if *entry.stop.borrow()||scope.is_cancelled(){return Err(TransportError::Cancelled);}
        let reject=||{entry.state.lock().queue_rejected+=1;invalid("connection queue full")};
        let regular=if matches!(request.priority,Priority::High){None}else{Some(entry.regular.clone().try_acquire_owned().map_err(|_|reject())?)};
        let slot=entry.slots.clone().try_acquire_owned().map_err(|_|reject())?;
        let (reply,mut rx)=oneshot::channel();
        let started=Arc::new(AtomicBool::new(false));
        let mut cancel=scope.cancelled.subscribe();
        {
            let _guard=scope.gate.lock();
            if scope.is_cancelled(){return Err(TransportError::Cancelled);}
            entry.tx.try_send(Job{request,queued:Instant::now(),reply,_slot:slot,_regular:regular,scope:scope.clone(),started:started.clone(),validator})
                .map_err(|e|match e {mpsc::error::TrySendError::Full(_)=>reject(),mpsc::error::TrySendError::Closed(_)=>TransportError::Cancelled})?;
        }
        tokio::select!{biased;
            result=&mut rx=>result.map_err(|_|TransportError::Cancelled),
            // Drop the watch::Ref inside the future; select must not retain
            // its read guard across the await of an active transaction.
            _=async {let _=cancel.wait_for(|v|*v).await;}=>{
                if started.load(Ordering::Acquire){rx.await.map_err(|_|TransportError::Cancelled)}
                else{Err(TransportError::Cancelled)}
            }
        }
    }

    /// Stop outside the queue. Keep the lease until the worker has exited,
    /// including when a caller cancels its disconnect future.
    pub async fn disconnect(&self,id:&str)->TransportResult<()>{
        let entry=self.entry(id)?;
        self.disconnect_entry(entry).await
    }
    async fn disconnect_entry(&self,entry:Arc<Entry>)->TransportResult<()>{
        // Pointer identity prevents an old cleanup task from deleting a newer
        // worker even if an explicit connection ID was reused.
        entry.stop.send_replace(true);
        let id=&entry.definition.id;
        let mut task=entry.task.lock().await;
        let joined=if let Some(handle)=task.as_mut(){handle.await.map_err(|e|TransportError::Other(e.to_string()))}else{Ok(())};
        task.take();
        let mut entries=self.entries.lock();
        if entries.get(id).is_some_and(|e|Arc::ptr_eq(e,&entry)){entries.remove(id);}
        joined
    }
}

pub struct SerialLease{exclusive:Arc<parking_lot::Mutex<HashSet<String>>>,resources:HashSet<String>}
impl Drop for SerialLease{
    fn drop(&mut self){let mut owned=self.exclusive.lock();for resource in &self.resources{owned.remove(resource);}}
}

pub struct TemporaryConnection {
    manager:Arc<ConnectionManager>,
    id:String,
    scope:Arc<OperationScope>,
    closed:bool,
}
impl TemporaryConnection{
    pub async fn transact(&self,request:TransactionRequest,validator:Option<ResponseValidator>)->TransportResult<TransactionOutcome>{
        self.manager.transact_scoped(&self.id,request,self.scope.clone(),validator).await
    }
    pub async fn close(mut self)->TransportResult<()>{
        self.scope.stop().await;
        let result=self.manager.disconnect(&self.id).await;
        self.closed=true;
        result
    }
}
impl Drop for TemporaryConnection{
    fn drop(&mut self){
        if self.closed{return;}
        self.scope.cancel();
        // temporary() required a Tokio runtime to register. If the runtime
        // is already shutting down, dropping the manager also signals workers.
        if let Ok(runtime)=tokio::runtime::Handle::try_current(){
            let manager=self.manager.clone();let id=self.id.clone();
            runtime.spawn(async move{let _=manager.disconnect(&id).await;});
        }
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

/// Recovery interval for formats without transaction IDs. A finite quiet time
/// reduces, but cannot eliminate, ambiguity from arbitrarily late slave replies.
fn recovery_pause(definition:&ConnectionDefinition,timeout:Duration)->Duration{
    let gap=match &definition.transport{
        TransportConfig::Serial{config,mode}=>{
            let gap=if *mode==SerialMode::Rtu{super::serial::required_rtu_gap(config.baud_rate,config.data_bits,config.parity,config.stop_bits)}else{Duration::from_millis(2)};
            gap.max(config.frame_gap)
        },
        TransportConfig::Tcp(config) if config.framing==TcpFraming::Rtu=>Duration::from_millis(2),
        TransportConfig::Udp{mbap:false,..}=>Duration::from_millis(2),
        _=>return Duration::ZERO,
    };
    let configured=definition.policy.recovery_ms.map(|ms|Duration::from_millis(u64::from(ms))).unwrap_or(timeout);
    configured.max(gap)
}
fn cancelled(job:Job){
    let _=job.reply.send(TransactionOutcome{result:Err(TransportError::Cancelled),timing:TransactionTiming::default(),queue_wait_us:micros(job.queued),rejected_response:None});
}

async fn worker(
    definition:ConnectionDefinition,factory:Arc<dyn TransportFactory>,
    state:Arc<parking_lot::Mutex<ConnectionRuntimeState>>,mut rx:mpsc::Receiver<Job>,mut stop:watch::Receiver<bool>,
    resource:String,recovery_book:Arc<RecoveryBook>,
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
        let Job{request,queued,mut reply,_slot,_regular,scope,started,validator}=job;
        if reply.is_closed(){continue;}
        let Some(_active)=scope.begin(&started) else {let _=reply.send(TransactionOutcome{result:Err(TransportError::Cancelled),timing:TransactionTiming::default(),queue_wait_us:micros(queued),rejected_response:None});continue;};
        drop(_slot);drop(_regular);
        let queue_wait_us=micros(queued);
        state.lock().active_requests=1;
        let mut timing=TransactionTiming::default();
        let mut rejected_response=None;
        let result=async{
            if transport.is_none(){
                if faulted&&!definition.policy.auto_reconnect {
                    return Err(invalid("connection faulted; disconnect and register again"));
                }
                let until=retry_after.into_iter().chain(recovery_book.deadline(&resource)).max();
                if let Some(until)=until{
                    set_status(&state,ConnectionStatus::RetryWait);
                    tokio::select!{biased;
                        _=stop.changed()=>return Err(TransportError::Cancelled),
                        _=reply.closed()=>return Err(TransportError::Cancelled),
                        _=async {let mut signal=scope.cancelled.subscribe();let _=signal.wait_for(|v|*v).await;}=>return Err(TransportError::Cancelled),
                        _=tokio::time::sleep_until(until)=>{}
                    }
                }
                if *stop.borrow() || reply.is_closed() || scope.is_cancelled(){return Err(TransportError::Cancelled);}
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
            if *stop.borrow() || reply.is_closed() || scope.is_cancelled(){return Err(TransportError::Cancelled);}
            let t=transport.as_mut().ok_or(TransportError::Closed)?;
            // Bound reset/TX as well as RX; the existing response timeout still applies.
            let budget=request.response_timeout.saturating_add(Duration::from_secs(2));
            match tokio::time::timeout(budget,t.transact_measured(&request.adu,request.response_timeout,request.expected_hint,&mut timing)).await{
                Ok(result)=>{
                    let bytes=result?;
                    if let Some(validate)=validator{
                        let start=Instant::now();
                        let checked=validate(&bytes);
                        timing.validation_us=Some(micros(start));
                        if let Err(error)=checked{timing.failure_stage=Some("validation".into());rejected_response=Some(bytes);return Err(error);}
                    }
                    Ok(bytes)
                },
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
                let recovery=if timing.tx_us.is_some()||timing.tx_complete||timing.failure_stage.as_deref()==Some("operation_deadline"){recovery_pause(&definition,request.response_timeout)}else{Duration::ZERO};
                let until=tokio::time::Instant::now()+backoff.max(recovery);
                if !recovery.is_zero(){recovery_book.record(&resource,until);}
                retry_after=Some(until);
                backoff=(backoff*2).min(MAX_BACKOFF);
                let mut s=state.lock();
                s.status=ConnectionStatus::Faulted;s.last_error=Some(e.to_string());
            }
        }
        state.lock().active_requests=0;
        let _=reply.send(TransactionOutcome{result,timing,queue_wait_us,rejected_response});
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

//! Typed live register monitor (R01). Existing serial/IP acquisition policy is
//! retained; observations and bounded history are independent of the UI timer.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use crate::communication::service::{CommunicationService,ProtocolRequest,framing};
use crate::communication::configuration::{network_definition,serial_definition,NetworkProtocol};
use crate::transport::connection::{ConnectionLease,ConnectionDefinition,Priority};
use tokio::sync::{mpsc, Mutex};

use crate::modbus::common::TransportKind;
use crate::modbus::frame::request;
use crate::transport::TransportError;
use crate::transport::timing::{micros, TransactionTiming};
use super::metrics::Metrics;
use crate::modbus::frame::Pdu;
use super::measurement::{Channel, DEFAULT_HISTORY, MAX_TOTAL_SAMPLES, UI_BATCH_MS};

use super::engine::verify_adu;
use super::EventSink;
use super::types::{
    now_ms, DataQuality, DeviceTarget, MonitorUpdate, MonitorValueState, MonitorWatch, register_width,
};

// Legacy DeviceTarget defaults remain until C.3.1 IPC propagation is complete.
fn monitor_definition(target:&DeviceTarget)->Result<ConnectionDefinition,TransportError>{
    match target.kind {
        TransportKind::Rtu|TransportKind::Ascii=>{
            use crate::modbus::common::SerialMode;
            let mode=if target.kind==TransportKind::Ascii{SerialMode::Ascii}else{SerialMode::Rtu};
            if target.serial_mode.is_some_and(|value|value!=mode){
                return Err(TransportError::Other("serial framing mismatch".into()));
            }
            let port=target.serial.as_ref().ok_or_else(||TransportError::Other("serial params required".into()))?;
            Ok(serial_definition("Monitor",port,mode))
        },
        TransportKind::Tcp=>network_definition("Monitor",&target.endpoint,target.port,NetworkProtocol::TcpMbap,None),
        TransportKind::Udp=>network_definition("Monitor",&target.endpoint,target.port,NetworkProtocol::UdpRtu,None),
    }
}

/// Control messages for the monitor task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMsg {
    Stop,
}

/// Cheap cloneable handle stored in Tauri state.
pub struct MonitorHandle {
    communication: Arc<CommunicationService>,
    control_tx: Mutex<Option<mpsc::Sender<ControlMsg>>>,
    values: Arc<RwLock<HashMap<String, Channel>>>,
}

impl Default for MonitorHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorHandle {
    pub fn new() -> Self {Self::with_communication(Arc::new(CommunicationService::default()))}
    pub fn with_communication(communication:Arc<CommunicationService>)->Self {
        Self {
            communication,
            control_tx: Mutex::new(None),
            values: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn snapshot(&self) -> Vec<MonitorValueState> {
        let mut values: Vec<_> = self.values.read().values().map(Channel::snapshot).collect();
        values.sort_by(|a, b| a.key.cmp(&b.key));
        values
    }

    /// Start (or restart) monitoring with the given watches and interval.
    pub async fn start(
        &self,
        sink: EventSink,
        watches: Vec<MonitorWatch>,
        interval_ms: u32,
        history_capacity: Option<u32>,
    ) -> Result<String, String> {
        let capacity = history_capacity.unwrap_or(DEFAULT_HISTORY);
        if watches.is_empty() || !(100..=50_000).contains(&capacity) || watches.len().saturating_mul(capacity as usize) > MAX_TOTAL_SAMPLES {
            return Err("history capacity must be 100..50000 per channel and total <= 1000000 observations".into());
        }
        let mut keys = std::collections::HashSet::new();
        for watch in &watches {
            let space = super::types::AddressSpace::from_read_function(watch.function)?;
            super::types::RegisterSpan {
                start: super::types::RegisterKey {
                    device_key: watch.target.endpoint.clone(), address_space: space, address: watch.address,
                },
                quantity: watch.quantity,
            }.validate()?;
            if let Some(spec) = &watch.spec {
                if register_width(spec.data_type) != watch.quantity ||
                    (space.is_bit_space() && spec.data_type != crate::modbus::decode::DataType::Uint16) ||
                    !spec.multiplier.is_finite() || !spec.divider.is_finite() || spec.divider == 0.0 || !spec.offset.is_finite() {
                    return Err("invalid monitor decode specification".into());
                }
            } else if watch.quantity != 1 {
                return Err("RAW watch requires one word; use a typed tag for a multiword span".into());
            }
            if watch.key.trim().is_empty() || !keys.insert(&watch.key) {
                return Err("empty or duplicate monitor watch key".into());
            }
        }
        let interval_ms = interval_ms.clamp(250, 10_000);
        {
            let mut guard = self.control_tx.lock().await;
            if let Some(tx) = guard.as_ref() {
                if !tx.is_closed() {
                    return Err("monitor already running".into());
                }
            }
            // Preflight before publishing history or spawning the scheduler.
            // Acquiring a lease registers a lazy worker; no device I/O occurs here.
            let mut leases=Vec::with_capacity(watches.len());
            for watch in &watches {
                let definition=monitor_definition(&watch.target).map_err(|e|e.to_string())?;
                leases.push(self.communication.manager.acquire(definition).map_err(|e|e.to_string())?);
            }
            let (tx, rx) = mpsc::channel(4);
            *guard = Some(tx);

            let session_id = format!("{:032x}", rand::random::<u128>());
            let mut values = HashMap::new();
            for w in &watches {
                values.insert(w.key.clone(), Channel::new(session_id.clone(), w.clone(), capacity));
            }
            *self.values.write() = values;

            tokio::spawn(monitor_task(sink, watches, interval_ms, rx, self.values.clone(), session_id.clone(), leases));
            Ok(session_id)
        }
    }

    pub async fn stop(&self) -> Result<(), String> {
        let mut guard = self.control_tx.lock().await;
        if let Some(tx) = guard.take() {
            if !tx.is_closed() {
                let _ = tx.send(ControlMsg::Stop).await;
                tx.closed().await;
            }
        }
        Ok(())
    }
}

async fn monitor_task(
    sink: EventSink,
    watches: Vec<MonitorWatch>,
    interval_ms: u32,
    mut control_rx: mpsc::Receiver<ControlMsg>,
    values: Arc<RwLock<HashMap<String, Channel>>>,
    session_id: String,
    leases: Vec<ConnectionLease>,
) {
    let interval = Duration::from_millis(interval_ms as u64);

    let started = Instant::now();
    let metrics = Arc::new(RwLock::new(Metrics::new(interval_ms)));
    let ui_metrics = metrics.clone();
    let (done_tx, mut done_rx) = tokio::sync::oneshot::channel::<()>();
    let ui_values = values.clone();
    let ui_sink = sink.clone();
    let ui_session = session_id.clone();
    let ui_task = tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(UI_BATCH_MS));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        tick.tick().await;
        loop {
            tokio::select! {
                _ = &mut done_rx => break,
                _ = tick.tick() => emit_delta(&ui_sink, &ui_values, &ui_metrics, micros(started), &ui_session, true),
            }
        }
    });
    'poll: loop {
        tokio::select! {
            msg = control_rx.recv() => {
                if matches!(msg, Some(ControlMsg::Stop) | None) { break; }
            }
            _ = tokio::time::sleep(interval) => {}
        }

        let cycle_start = Instant::now();
        metrics.write().begin_cycle(micros(started));
        for (watch,lease) in watches.iter().zip(&leases) {
            if !matches!(control_rx.try_recv(), Err(mpsc::error::TryRecvError::Empty)) { break 'poll; }
            let acquisition_start = Instant::now();
            let mut timing = TransactionTiming::default();
            let pdu = match watch.function {
                0x01 => request::read_coils(watch.address, watch.quantity),
                0x02 => request::read_discrete_inputs(watch.address, watch.quantity),
                0x03 => request::read_holding_registers(watch.address, watch.quantity),
                0x04 => request::read_input_registers(watch.address, watch.quantity),
                _ => continue,
            };
            let (kind,engine)=framing(lease.definition());
            let request=ProtocolRequest{unit:watch.target.unit_id,pdu,timeout:Duration::from_millis(u64::from(interval_ms.min(1000))),priority:Priority::Normal};
            let (result,tx)=match CommunicationService::execute_leased(lease,request).await {
                Ok(out)=>{
                    timing=out.transaction.timing;
                    (out.transaction.result,out.transaction_id)
                },
                Err(error)=>(Err(error),0),
            };
            let parsed = result.map_err(classify_transport).and_then(|resp| {
                let pdu = verify_adu(kind, watch.target.unit_id, watch.function, &resp, tx, engine.clone())
                    .map_err(classify_transport)?;
                match pdu {
                    Pdu::Exception { code, .. } => Err((DataQuality::ModbusException, Some(code), format!("Modbus exception 0x{code:02X}"))),
                    Pdu::Response { data, .. } => crate::modbus::frame::response::parse_read_values(watch.function, &data, watch.quantity)
                        .map_err(|e| (DataQuality::InvalidResponse, None, e.to_string())),
                    _ => Err((DataQuality::InvalidResponse, None, "unexpected PDU".into())),
                }
            });
            let record = {
                let mut all = values.write();
                let record = all.get_mut(&watch.key).map(|channel| channel.observe_timed(now_ms(), micros(started), parsed, timing, acquisition_start));
                if let Some(r) = &record { metrics.write().observe(watch, &r.sample); }
                record
            };
            if let Some(record) = record { sink.samples(&[record]); }
        }
        metrics.write().complete_cycle(micros(cycle_start));
    }

    for lease in &leases {lease.stop().await;}
    drop(leases);
    let _ = done_tx.send(());
    let _ = ui_task.await;
    emit_delta(&sink, &values, &metrics, micros(started), &session_id, false);
}

fn classify_transport(error: TransportError) -> (DataQuality, Option<u8>, String) {
    let quality = match &error {
        TransportError::Timeout(_) => DataQuality::Timeout,
        TransportError::Modbus(_) => DataQuality::InvalidResponse,
        _ => DataQuality::TransportError,
    };
    (quality, None, error.to_string())
}

fn emit_delta(sink: &EventSink, values: &RwLock<HashMap<String, Channel>>, metrics: &RwLock<Metrics>, elapsed: u64, session_id: &str, running: bool) {
    let (mut items, snapshot) = {
        let mut channels = values.write();
        let items: Vec<_> = channels.values_mut().filter_map(Channel::delta).collect();
        (items, metrics.read().snapshot(elapsed,running))
    };
    items.sort_by(|a,b| a.key.cmp(&b.key));
    let update = MonitorUpdate { session_id: session_id.into(), values: items, running, metrics: snapshot };
    if let Ok(value) = serde_json::to_value(&update) { sink.emit("monitor:update", value); }
}

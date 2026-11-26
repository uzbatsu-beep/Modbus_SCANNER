//! Typed live register monitor (R01). Existing serial/IP acquisition policy is
//! retained; observations and bounded history are independent of the UI timer.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::{mpsc, Mutex};

use crate::modbus::common::TransportKind;
use crate::modbus::frame::request;
use crate::transport::serial::SerialTransport;
use crate::transport::tcp::TcpConfig;
use crate::transport::tcp::TcpTransport;
use crate::transport::udp::UdpTransport;
use crate::transport::{Transport, TransportError};
use crate::transport::timing::{micros, TransactionTiming};
use super::metrics::Metrics;
use crate::modbus::frame::Pdu;
use super::measurement::{Channel, DEFAULT_HISTORY, MAX_TOTAL_SAMPLES, UI_BATCH_MS};

use super::engine::{
    build_adu, ensure_serial_settings, next_transaction_id, verify_adu, EngineParams,
};
use super::EventSink;
use super::types::{
    now_ms, DataQuality, MonitorUpdate, MonitorValueState, MonitorWatch, ScanParams, register_width,
};

/// Control messages for the monitor task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMsg {
    Stop,
}

/// Cheap cloneable handle stored in Tauri state.
pub struct MonitorHandle {
    control_tx: Mutex<Option<mpsc::Sender<ControlMsg>>>,
    values: Arc<RwLock<HashMap<String, Channel>>>,
}

impl Default for MonitorHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorHandle {
    pub fn new() -> Self {
        Self {
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
            let (tx, rx) = mpsc::channel(4);
            *guard = Some(tx);

            let session_id = format!("{:032x}", rand::random::<u128>());
            let mut values = HashMap::new();
            for w in &watches {
                values.insert(w.key.clone(), Channel::new(session_id.clone(), w.clone(), capacity));
            }
            *self.values.write() = values;

            let mut defaults = ScanParams::default();
            defaults.timeout_ms = interval_ms.min(1000);
            defaults.retries = 0;
            let engine = EngineParams::from_scan_params(&defaults);

            tokio::spawn(monitor_task(sink, watches, interval_ms, rx, self.values.clone(), engine, session_id.clone()));
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
    engine: EngineParams,
    session_id: String,
) {
    let mut serial_transport: Option<SerialTransport> = None;
    let mut serial_signature: Option<String> = None;
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
        for watch in &watches {
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
            if matches!(watch.target.kind, TransportKind::Rtu | TransportKind::Ascii) {
                ensure_serial_settings(&mut serial_transport, &mut serial_signature, &watch.target).await;
            }
            let tx = next_transaction_id();
            let adu = build_adu(watch.target.kind, watch.target.unit_id, &pdu, engine.clone(), tx);
            let timeout = engine.timeout;
            let hint = 3 + (watch.quantity as usize) * 2;

            let result: Result<Vec<u8>, TransportError> = match watch.target.kind {
                TransportKind::Rtu | TransportKind::Ascii => match serial_transport.as_mut() {
                    Some(t) => t.transact_measured(&adu, timeout, hint, &mut timing).await,
                    None => match watch.target.serial.clone() {
                        Some(sp) => {
                            let cfg = crate::transport::serial::SerialPortConfig {
                                port_name: sp.port_name,
                                baud_rate: sp.baud_rate,
                                data_bits: sp.data_bits.into(),
                                parity: sp.parity.into(),
                                stop_bits: sp.stop_bits.into(),
                                frame_gap: Duration::from_millis(2),
                            };
                            let mode = match watch.target.serial_mode {
                                Some(crate::modbus::common::SerialMode::Ascii) => {
                                    crate::modbus::common::SerialMode::Ascii
                                }
                                _ => crate::modbus::common::SerialMode::Rtu,
                            };
                            let connect_start = Instant::now();
                            let opened = SerialTransport::open(cfg, mode).await;
                            timing.connect_us = Some(micros(connect_start));
                            match opened {
                                Ok(t) => {
                                    serial_transport = Some(t);
                                    serial_transport
                                        .as_mut()
                                        .unwrap()
                                        .transact_measured(&adu, timeout, hint, &mut timing)
                                        .await
                                }
                                Err(e) => { timing.failure_stage = Some("connect".into()); Err(e) },
                            }
                        }
                        None => { timing.failure_stage = Some("connect".into()); Err(TransportError::Other("serial params required".into())) },
                    },
                },
                TransportKind::Tcp => {
                    let connect_start = Instant::now();
                    let connected = TcpTransport::connect(TcpConfig {
                    host: watch.target.endpoint.clone(),
                    port: watch.target.port.unwrap_or(502),
                    connect_timeout: timeout,
                    framing: engine.tcp_framing,
                tls: engine.tls.clone(),
                })
                .await;
                    timing.connect_us = Some(micros(connect_start));
                    match connected {
                        Ok(mut t) => {
                            let out = t.transact_measured(&adu, timeout, hint, &mut timing).await;
                            let _ = t.close().await;
                            out
                        }
                        Err(e) => { timing.failure_stage = Some("connect".into()); Err(e) },
                    }
                },
                TransportKind::Udp => {
                    let connect_start = Instant::now();
                    let connected = UdpTransport::connect(
                    &watch.target.endpoint,
                    watch.target.port.unwrap_or(502),
                )
                .await;
                    timing.connect_us = Some(micros(connect_start));
                    match connected {
                        Ok(mut t) => t.transact_measured(&adu, timeout, hint, &mut timing).await,
                        Err(e) => { timing.failure_stage = Some("connect".into()); Err(e) },
                    }
                },
            };

            let validation_start = Instant::now();
            let received = result.is_ok();
            let parsed = result.map_err(classify_transport).and_then(|resp| {
                let pdu = verify_adu(watch.target.kind, watch.target.unit_id, watch.function, &resp, tx, engine.clone())
                    .map_err(classify_transport)?;
                match pdu {
                    Pdu::Exception { code, .. } => Err((DataQuality::ModbusException, Some(code), format!("Modbus exception 0x{code:02X}"))),
                    Pdu::Response { data, .. } => crate::modbus::frame::response::parse_read_values(watch.function, &data, watch.quantity)
                        .map_err(|e| (DataQuality::InvalidResponse, None, e.to_string())),
                    _ => Err((DataQuality::InvalidResponse, None, "unexpected PDU".into())),
                }
            });
            if received {
                timing.validation_us = Some(micros(validation_start));
                if matches!(&parsed, Err((DataQuality::InvalidResponse, _, _))) { timing.failure_stage = Some("validation".into()); }
            }
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

    if let Some(mut t) = serial_transport.take() {
        let _ = t.close().await;
    }
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

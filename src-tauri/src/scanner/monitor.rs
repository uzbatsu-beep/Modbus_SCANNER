//! Live register monitor (P3-05).
//!
//! Polls a set of registered watches at a fixed interval, keeps a bounded
//! sample history per watch and emits `monitor:update` with min/max and change
//! flags after every cycle. Serial watches reuse one open port; IP watches use
//! a fresh transport per poll (same policy as the scanner).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tauri::Emitter;
use tokio::sync::{mpsc, Mutex};

use crate::modbus::common::TransportKind;
use crate::modbus::frame::request;
use crate::transport::serial::SerialTransport;
use crate::transport::tcp::TcpConfig;
use crate::transport::tcp::TcpTransport;
use crate::transport::udp::UdpTransport;
use crate::transport::Transport;

use super::engine::{
    build_adu, ensure_serial_settings, next_transaction_id, verify_adu, EngineParams,
};
use super::types::{
    now_ms, MonitorSample, MonitorUpdate, MonitorValueState, MonitorWatch, ScanParams,
};

/// Control messages for the monitor task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMsg {
    Stop,
}

/// Cheap cloneable handle stored in Tauri state.
pub struct MonitorHandle {
    control_tx: Mutex<Option<mpsc::Sender<ControlMsg>>>,
    values: Arc<RwLock<HashMap<String, MonitorValueState>>>,
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
        let mut values: Vec<_> = self.values.read().values().cloned().collect();
        values.sort_by(|a, b| a.key.cmp(&b.key));
        values
    }

    /// Start (or restart) monitoring with the given watches and interval.
    pub async fn start(
        &self,
        app: tauri::AppHandle,
        watches: Vec<MonitorWatch>,
        interval_ms: u32,
    ) -> Result<(), String> {
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

            let mut values = HashMap::new();
            for w in &watches {
                values.insert(
                    w.key.clone(),
                    MonitorValueState {
                        key: w.key.clone(),
                        name: w.name.clone().unwrap_or_else(|| w.key.clone()),
                        unit: w.unit.clone().unwrap_or_default(),
                        values: Vec::new(),
                        min: None,
                        max: None,
                        changed: false,
                        samples: Vec::new(),
                        errors: 0,
                    },
                );
            }
            *self.values.write() = values;

            let mut defaults = ScanParams::default();
            defaults.timeout_ms = interval_ms.min(1000);
            defaults.retries = 0;
            let engine = EngineParams::from_scan_params(&defaults);

            tokio::spawn(monitor_task(app, watches, interval_ms, rx, self.values.clone(), engine));
            Ok(())
        }
    }

    pub async fn stop(&self) -> Result<(), String> {
        let guard = self.control_tx.lock().await;
        match guard.as_ref() {
            Some(tx) => tx.send(ControlMsg::Stop).await.map_err(|e| e.to_string()),
            None => Err("monitor not running".into()),
        }
    }
}

async fn monitor_task(
    app: tauri::AppHandle,
    watches: Vec<MonitorWatch>,
    interval_ms: u32,
    mut control_rx: mpsc::Receiver<ControlMsg>,
    values: Arc<RwLock<HashMap<String, MonitorValueState>>>,
    engine: EngineParams,
) {
    let mut serial_transport: Option<SerialTransport> = None;
    let mut serial_signature: Option<String> = None;
    let interval = Duration::from_millis(interval_ms as u64);

    loop {
        tokio::select! {
            msg = control_rx.recv() => {
                if matches!(msg, Some(ControlMsg::Stop) | None) { break; }
            }
            _ = tokio::time::sleep(interval) => {}
        }

        for watch in &watches {
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
            let adu = build_adu(watch.target.kind, watch.target.unit_id, &pdu, engine, tx);
            let timeout = engine.timeout;
            let hint = 3 + (watch.quantity as usize) * 2;

            let result: Result<Vec<u8>, String> = match watch.target.kind {
                TransportKind::Rtu | TransportKind::Ascii => match serial_transport.as_mut() {
                    Some(t) => t.transact(&adu, timeout, hint).await.map_err(|e| e.to_string()),
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
                            match SerialTransport::open(cfg, mode).await {
                                Ok(t) => {
                                    serial_transport = Some(t);
                                    serial_transport
                                        .as_mut()
                                        .unwrap()
                                        .transact(&adu, timeout, hint)
                                        .await
                                        .map_err(|e| e.to_string())
                                }
                                Err(e) => Err(e.to_string()),
                            }
                        }
                        None => Err("serial params required".into()),
                    },
                },
                TransportKind::Tcp => match TcpTransport::connect(TcpConfig {
                    host: watch.target.endpoint.clone(),
                    port: watch.target.port.unwrap_or(502),
                    connect_timeout: timeout,
                    framing: engine.tcp_framing,
                })
                .await
                {
                    Ok(mut t) => {
                        let out = t.transact(&adu, timeout, hint).await.map_err(|e| e.to_string());
                        let _ = t.close().await;
                        out
                    }
                    Err(e) => Err(e.to_string()),
                },
                TransportKind::Udp => match UdpTransport::connect(
                    &watch.target.endpoint,
                    watch.target.port.unwrap_or(502),
                )
                .await
                {
                    Ok(mut t) => t.transact(&adu, timeout, hint).await.map_err(|e| e.to_string()),
                    Err(e) => Err(e.to_string()),
                },
            };

            let parsed = result.ok().and_then(|resp| {
                let pdu = verify_adu(watch.target.kind, watch.target.unit_id, watch.function, &resp, tx, engine).ok()?;
                if pdu.is_exception() {
                    return None;
                }
                let data = match &pdu {
                    crate::modbus::frame::Pdu::Response { data, .. } => data.clone(),
                    _ => return None,
                };
                let values = match watch.function {
                    0x03 | 0x04 => crate::modbus::frame::response::parse_registers(&data).ok()?,
                    0x01 | 0x02 => crate::modbus::frame::response::parse_bits(&data, watch.quantity)
                        .ok()?
                        .into_iter()
                        .map(|b| b as u16)
                        .collect(),
                    _ => Vec::new(),
                };
                Some(values)
            });

            // Update in a closed scope; no guard crosses an await.
            {
                let mut all = values.write();
                if let Some(state) = all.get_mut(&watch.key) {
                    match parsed {
                        Some(values) => {
                            let previous = state.values.clone();
                            let sample = MonitorSample {
                                timestamp_ms: now_ms(),
                                values: values.clone(),
                                rtt_us: 0,
                            };
                            state.samples.push(sample);
                            let cutoff = now_ms().saturating_sub(3_600_000);
                            state.samples.retain(|s| s.timestamp_ms >= cutoff);
                            if state.samples.len() > 5_000 {
                                state.samples.drain(..state.samples.len() - 5_000);
                            }
                            for &v in &values {
                                state.min = Some(state.min.map_or(v, |m| m.min(v)));
                                state.max = Some(state.max.map_or(v, |m| m.max(v)));
                            }
                            state.changed = previous != values;
                            state.values = values;
                        }
                        None => state.errors += 1,
                    }
                }
            }
        }

        let snapshot: Vec<_> = {
            let all = values.read();
            let mut items: Vec<_> = all.values().cloned().collect();
            items.sort_by(|a, b| a.key.cmp(&b.key));
            items
        };
        let _ = app.emit("monitor:update", &MonitorUpdate {
            values: snapshot,
            cycle_ms: interval_ms as u64,
        });
    }

    if let Some(mut t) = serial_transport.take() {
        let _ = t.close().await;
    }
}

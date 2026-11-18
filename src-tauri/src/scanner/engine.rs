//! Scan engine: async task management, event emission, pause/resume/stop.
//!
//! Design:
//! - [`ScannerHandle`] is a cheap cloneable handle stored in the Tauri state;
//! - `start_scan` spawns a tokio task that consumes work items from a strategy;
//! - serial work is strictly sequential (one bus!); IP work is bounded-concurrent;
//! - control is done via an mpsc [`ControlMsg`] channel + a watch channel for pause;
//! - results are shared via [`Arc<RwLock<ScanResults>>`] and pushed to the
//!   frontend as Tauri events (`scan:progress`, `scan:event`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::{mpsc, watch, Mutex};

use crate::modbus::common::{ModbusError, SerialMode, TransportKind, UnitId};
use crate::modbus::frame::{parse_device_identification, read_device_identification, Pdu};
use crate::transport::serial::{SerialPortConfig, SerialTransport};
use crate::transport::tcp::{TcpFraming, TcpTransport};
use crate::transport::udp::UdpTransport;
use crate::transport::{Transport, TransportError};

use super::strategies::{build_strategy, RegisterReadOutcome, ScanStrategy, WorkItem};
use super::types::*;

/// Control messages to the engine task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMsg {
    Pause,
    Resume,
    Stop,
}

/// Effective timing/limits derived from [`ScanParams`].
///
/// All values are clamped once at scan start, so the engine task and its workers
/// use identical settings for every request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineParams {
    /// Response/connect timeout for a single request.
    pub timeout: Duration,
    /// Additional attempts after the first one (0 = no retries).
    pub retries: u32,
    /// Delay between consecutive work items.
    pub request_gap: Duration,
    /// Max in-flight IP probes (serial work is always sequential).
    pub ip_concurrency: usize,
    /// Whether to attempt FC43/MEI14 identification after a device answers.
    pub identify_devices: bool,
    /// TCP wire format (MBAP or RTU over TCP).
    pub tcp_framing: TcpFraming,
    /// MBAP framing for UDP instead of the default RTU-style ADU.
    pub udp_mbap: bool,
}

impl EngineParams {
    pub fn from_scan_params(p: &ScanParams) -> Self {
        Self {
            timeout: Duration::from_millis(p.timeout_ms.clamp(50, 10_000) as u64),
            retries: p.retries.min(5),
            request_gap: Duration::from_millis(p.request_gap_ms.min(1_000) as u64),
            ip_concurrency: (p.ip_concurrency.clamp(1, 256)) as usize,
            identify_devices: p.identify_devices,
            tcp_framing: p.tcp_framing,
            udp_mbap: p.udp_mbap,
        }
    }
}

#[cfg(test)]
mod ip_class_tests {
    use super::*;

    #[test]
    fn ranking_prefers_positive_evidence() {
        assert!(IpTargetClass::ValidModbus.rank() > IpTargetClass::PortOpenNoModbus.rank());
        assert!(IpTargetClass::PortOpenNoModbus.rank() > IpTargetClass::ConnectionRefused.rank());
        assert!(IpTargetClass::ConnectionRefused.rank() > IpTargetClass::Timeout.rank());
        assert!(IpTargetClass::Timeout.rank() > IpTargetClass::Unreachable.rank());
    }

    #[test]
    fn maps_io_errors() {
        use std::io::ErrorKind;
        assert_eq!(
            classify_ip_error(&TransportError::Io(ErrorKind::ConnectionRefused.into())),
            IpTargetClass::ConnectionRefused
        );
        assert_eq!(
            classify_ip_error(&TransportError::Io(ErrorKind::HostUnreachable.into())),
            IpTargetClass::Unreachable
        );
        assert_eq!(
            classify_ip_error(&TransportError::Io(ErrorKind::TimedOut.into())),
            IpTargetClass::Timeout
        );
        assert_eq!(
            classify_ip_error(&TransportError::Timeout(Duration::from_millis(500))),
            IpTargetClass::Timeout
        );
    }
}

#[cfg(test)]
mod engine_params_tests {
    use super::*;

    #[test]
    fn maps_params_with_clamps() {
        let mut p = ScanParams::default();
        p.timeout_ms = 1_200;
        p.retries = 2;
        p.request_gap_ms = 25;
        p.ip_concurrency = 16;
        p.tcp_framing = TcpFraming::Rtu;
        p.udp_mbap = true;
        let e = EngineParams::from_scan_params(&p);
        assert_eq!(e.timeout, Duration::from_millis(1_200));
        assert_eq!(e.retries, 2);
        assert_eq!(e.request_gap, Duration::from_millis(25));
        assert_eq!(e.ip_concurrency, 16);
        assert!(e.identify_devices);
        assert_eq!(e.tcp_framing, TcpFraming::Rtu);
        assert!(e.udp_mbap);
    }

    #[test]
    fn clamps_out_of_range_values() {
        let mut p = ScanParams::default();
        p.timeout_ms = 0;
        p.retries = 99;
        p.request_gap_ms = 99_999;
        p.ip_concurrency = 0;
        let e = EngineParams::from_scan_params(&p);
        assert_eq!(e.timeout, Duration::from_millis(50));
        assert_eq!(e.retries, 5);
        assert_eq!(e.request_gap, Duration::from_millis(1_000));
        assert_eq!(e.ip_concurrency, 1);
        assert!(e.identify_devices);
        assert_eq!(e.tcp_framing, TcpFraming::Mbap);
        assert!(!e.udp_mbap);
    }
}

/// Shared engine state handle.
pub struct ScannerHandle {
    /// Control channel to the running task (None when idle).
    control_tx: Mutex<Option<mpsc::Sender<ControlMsg>>>,
    /// Current status (watch: UI reads it cheaply).
    status_tx: watch::Sender<ScanStatus>,
    /// Shared results.
    results: Arc<RwLock<ScanResults>>,
}

impl Default for ScannerHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl ScannerHandle {
    pub fn new() -> Self {
        let (status_tx, _) = watch::channel(ScanStatus::Idle);
        Self {
            control_tx: Mutex::new(None),
            status_tx,
            results: Arc::new(RwLock::new(ScanResults::default())),
        }
    }

    pub fn status(&self) -> ScanStatus {
        *self.status_tx.borrow()
    }

    /// Latest progress snapshot.
    pub fn progress(&self) -> ScanProgress {
        let results = self.results.read();
        let total = results.stats.requests_sent + results.stats.timeouts;
        ScanProgress {
            status: *self.status_tx.borrow(),
            total,
            done: total,
            found: results.records.len() as u64,
            current: String::new(),
            errors: results.stats.timeouts + results.stats.transport_errors,
            elapsed_ms: results.stats.elapsed_ms(),
        }
    }

    pub fn results(&self) -> ScanResults {
        self.results.read().clone()
    }

    pub fn clear_results(&self) {
        let mut r = self.results.write();
        *r = ScanResults::default();
    }

    /// Start a new scan. Returns an error if one is already running.
    pub async fn start(
        &self,
        app: tauri::AppHandle,
        target: ScanTarget,
        params: ScanParams,
    ) -> Result<(), String> {
        {
            let mut guard = self.control_tx.lock().await;
            if let Some(tx) = guard.as_ref() {
                if !tx.is_closed() && *self.status_tx.borrow() == ScanStatus::Running {
                    return Err("scan already in progress".into());
                }
            }
            let (tx, rx) = mpsc::channel(8);
            *guard = Some(tx);

            let (pause_tx, pause_rx) = watch::channel(false);
            let results = self.results.clone();
            let status_tx = self.status_tx.clone();

            let _ = self.status_tx.send(ScanStatus::Preparing);
            let _ = status_tx;

            let engine_params = EngineParams::from_scan_params(&params);
            let strategy = build_strategy(target, params)
                .map_err(|e| {
                    let _ = self.status_tx.send(ScanStatus::Failed);
                    e
                })?;

            *self.results.write() = ScanResults {
                records: Vec::new(),
                ip_targets: Vec::new(),
                stats: ScanStats {
                    started_ms: Some(now_ms()),
                    ..Default::default()
                },
            };

            tokio::spawn(engine_task(
                app,
                strategy,
                engine_params,
                rx,
                (pause_tx, pause_rx),
                results,
                status_tx,
            ));
            Ok(())
        }
    }

    pub async fn control(&self, msg: ControlMsg) -> Result<(), String> {
        let guard = self.control_tx.lock().await;
        match guard.as_ref() {
            Some(tx) => tx.send(msg).await.map_err(|e| e.to_string()),
            None => Err("no scan running".into()),
        }
    }
}

/// The engine task.
async fn engine_task(
    app: tauri::AppHandle,
    mut strategy: Box<dyn ScanStrategy + Send>,
    engine: EngineParams,
    mut control_rx: mpsc::Receiver<ControlMsg>,
    (pause_tx, mut pause_rx): (watch::Sender<bool>, watch::Receiver<bool>),
    results: Arc<RwLock<ScanResults>>,
    status_tx: watch::Sender<ScanStatus>,
) {
    let total = strategy.total();
    let mut done: u64 = 0;
    let mut serial_transport: Option<SerialTransport> = None;
    // Serial settings currently held by `serial_transport`; a sweep changes
    // them, so the port must be reopened when this signature changes.
    let mut serial_signature: Option<String> = None;
    // Register-plan work discovered during probes, executed by the engine so
    // that serial reads reuse the single open port.
    let mut pending: std::collections::VecDeque<WorkItem> = std::collections::VecDeque::new();
    // In-flight IP probes, bounded by `engine.ip_concurrency`.
    let mut ip_tasks: tokio::task::JoinSet<ProbeOutcome> = tokio::task::JoinSet::new();
    let mut want_stop = false;

    let _ = status_tx.send(ScanStatus::Running);
    emit_event(&app, &ScanEvent::StatusChanged {
        status: ScanStatus::Running,
        message: "scanning started".into(),
    });

    'main: loop {
        // Handle control messages (non-blocking).
        while let Ok(msg) = control_rx.try_recv() {
            match msg {
                ControlMsg::Stop => {
                    want_stop = true;
                    ip_tasks.abort_all();
                    let _ = status_tx.send(ScanStatus::Stopped);
                    break 'main;
                }
                ControlMsg::Pause => {
                    let _ = pause_tx.send(true);
                    let _ = status_tx.send(ScanStatus::Paused);
                    emit_event(&app, &ScanEvent::StatusChanged {
                        status: ScanStatus::Paused,
                        message: "scanning paused".into(),
                    });
                }
                ControlMsg::Resume => {
                    let _ = pause_tx.send(false);
                    let _ = status_tx.send(ScanStatus::Running);
                    emit_event(&app, &ScanEvent::StatusChanged {
                        status: ScanStatus::Running,
                        message: "scanning resumed".into(),
                    });
                }
            }
        }

        // Collect finished IP probes.
        while let Some(finished) = ip_tasks.try_join_next() {
            match finished {
                Ok(outcome) => {
                    let stop_on_found = strategy.stop_on_found();
                    apply_probe_outcome(&mut strategy, &mut pending, outcome, &mut want_stop, stop_on_found);
                    if want_stop {
                        break;
                    }
                }
                Err(e) if !e.is_cancelled() => tracing::warn!("IP probe task failed: {e}"),
                Err(_) => {}
            }
        }
        if want_stop {
            break;
        }

        // Wait while paused (already running IP probes finish).
        if *pause_rx.borrow() {
            tokio::select! {
                _ = pause_rx.changed() => {},
                msg = control_rx.recv() => {
                    if let Some(ControlMsg::Stop) = msg {
                        want_stop = true;
                        ip_tasks.abort_all();
                        let _ = status_tx.send(ScanStatus::Stopped);
                        break;
                    }
                }
            }
            continue;
        }

        // Next item: register work first, then the strategy's probes.
        let item = match pending.pop_front() {
            Some(i) => i,
            None => match strategy.next_item() {
                Some(i) => i,
                None => break,
            },
        };

        match item {
            WorkItem::Probe { target, function, pdu } => {
                if matches!(target.kind, TransportKind::Tcp | TransportKind::Udp) {
                    // Respect the cap strictly: wait for a free slot before spawning.
                    while ip_tasks.len() >= engine.ip_concurrency && !want_stop {
                        match ip_tasks.join_next().await {
                            Some(Ok(outcome)) => {
                                let stop_on_found = strategy.stop_on_found();
                                apply_probe_outcome(
                                    &mut strategy,
                                    &mut pending,
                                    outcome,
                                    &mut want_stop,
                                    stop_on_found,
                                );
                            }
                            Some(Err(e)) if !e.is_cancelled() => {
                                tracing::warn!("IP probe task failed: {e}");
                            }
                            _ => {}
                        }
                    }
                    if want_stop {
                        break;
                    }

                    // Each IP probe owns its transport: safe to run concurrently.
                    let task_app = app.clone();
                    let task_results = results.clone();
                    ip_tasks.spawn(async move {
                        let mut unused_serial = None;
                        execute_probe(
                            &task_app,
                            &mut unused_serial,
                            &target,
                            function,
                            &pdu,
                            &task_results,
                            engine,
                        )
                        .await
                    });
                } else {
                    // Serial probes must stay strictly sequential: one bus, one transport.
                    ensure_serial_settings(&mut serial_transport, &mut serial_signature, &target).await;
                    let outcome = execute_probe(
                        &app,
                        &mut serial_transport,
                        &target,
                        function,
                        &pdu,
                        &results,
                        engine,
                    )
                    .await;
                    let stop_on_found = strategy.stop_on_found();
                    apply_probe_outcome(&mut strategy, &mut pending, outcome, &mut want_stop, stop_on_found);
                }
                done += 1;
                emit_progress(&app, *status_tx.borrow(), total, done, &results);
            }
            item @ WorkItem::RegisterRead { .. } => {
                if let WorkItem::RegisterRead { target, .. } = &item {
                    ensure_serial_settings(&mut serial_transport, &mut serial_signature, target).await;
                }
                let outcome = execute_work(&app, &item, &mut serial_transport, &results, engine).await;
                if let Some(outcome) = outcome {
                    for follow in strategy.on_register_read(&item, outcome) {
                        pending.push_back(follow);
                    }
                }
                done += 1;
                emit_progress(&app, *status_tx.borrow(), total, done, &results);
            }
        }

        // Inter-request gap from ScanParams (applies to all work items).
        if !engine.request_gap.is_zero() {
            tokio::time::sleep(engine.request_gap).await;
        }
    }

    // Wait for the remaining in-flight probes (already counted in `done`).
    while let Some(finished) = ip_tasks.join_next().await {
        if let Ok(outcome) = finished {
            let stop_on_found = strategy.stop_on_found();
            apply_probe_outcome(&mut strategy, &mut pending, outcome, &mut want_stop, stop_on_found);
        }
    }
    // Execute any register work collected from the last probes.
    while let Some(item) = pending.pop_front() {
        if let WorkItem::RegisterRead { target, .. } = &item {
            ensure_serial_settings(&mut serial_transport, &mut serial_signature, target).await;
        }
        let outcome = execute_work(&app, &item, &mut serial_transport, &results, engine).await;
        if let Some(outcome) = outcome {
            for follow in strategy.on_register_read(&item, outcome) {
                pending.push_back(follow);
            }
        }
    }

    // Cleanup.
    if let Some(mut t) = serial_transport.take() {
        let _ = t.close().await;
    }

    let status = if want_stop { ScanStatus::Stopped } else { ScanStatus::Finished };
    {
        let mut r = results.write();
        r.stats.finished_ms = Some(now_ms());
    }
    let _ = status_tx.send(status);

    let r = results.read();
    emit_event(&app, &ScanEvent::Finished {
        total: done,
        found: r.records.len() as u64,
        errors: r.stats.timeouts + r.stats.transport_errors,
        elapsed_ms: r.stats.elapsed_ms(),
    });
    let _ = status_tx;
}

/// Schedule the register plan for a found device and honour `stop_on_first`.
fn apply_probe_outcome(
    strategy: &mut Box<dyn ScanStrategy + Send>,
    pending: &mut std::collections::VecDeque<WorkItem>,
    outcome: ProbeOutcome,
    want_stop: &mut bool,
    stop_on_found: bool,
) {
    if outcome.is_found {
        for work in strategy.on_device_found(&outcome.target) {
            pending.push_back(work);
        }
        if stop_on_found {
            *want_stop = true;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Ok,
    Failed,
}

/// What a completed probe hands back to the engine.
#[derive(Debug)]
struct ProbeOutcome {
    /// True when the device answered (data or exception).
    is_found: bool,
    /// Kept so the engine can schedule the register plan for a found device.
    target: DeviceTarget,
}

/// Next MBAP transaction id (shared by all concurrent IP probes).
fn next_transaction_id() -> u16 {
    static NEXT: AtomicU16 = AtomicU16::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn mbap_frame(unit_id: u8, pdu: &[u8], transaction_id: u16) -> Vec<u8> {
    let mut frame = Vec::with_capacity(7 + pdu.len());
    frame.extend_from_slice(&transaction_id.to_be_bytes());
    frame.extend_from_slice(&0u16.to_be_bytes()); // protocol id
    frame.extend_from_slice(&((1 + pdu.len()) as u16).to_be_bytes());
    frame.push(unit_id);
    frame.extend_from_slice(pdu);
    frame
}

/// Verify an MBAP response: transaction id, protocol id, length, unit id.
fn verify_mbap(adu: &[u8], expected_unit: u8, expected_tx: u16) -> Result<Pdu, TransportError> {
    if adu.len() < 8 {
        return Err(TransportError::Modbus(ModbusError::TooShort(adu.len())));
    }
    let transaction_id = u16::from_be_bytes([adu[0], adu[1]]);
    if transaction_id != expected_tx {
        return Err(TransportError::Modbus(ModbusError::TransactionIdMismatch {
            expected: expected_tx,
            actual: transaction_id,
        }));
    }
    let protocol_id = u16::from_be_bytes([adu[2], adu[3]]);
    if protocol_id != 0 {
        return Err(TransportError::Other(format!("bad protocol id {protocol_id}")));
    }
    let length = u16::from_be_bytes([adu[4], adu[5]]) as usize;
    if adu.len() != 6 + length {
        return Err(TransportError::Modbus(ModbusError::InvalidFrame(format!(
            "MBAP length {length} but {} bytes received",
            adu.len() - 6
        ))));
    }
    let unit_id = adu[6];
    if unit_id != expected_unit {
        return Err(TransportError::Modbus(ModbusError::UnitIdMismatch {
            expected: expected_unit,
            actual: unit_id,
        }));
    }
    Pdu::from_bytes(&adu[7..]).map_err(TransportError::Modbus)
}

/// Build the wire ADU, honouring the selected TCP/UDP framing.
fn build_adu(kind: TransportKind, unit_id: u8, pdu: &[u8], engine: EngineParams, tx: u16) -> Vec<u8> {
    match kind {
        TransportKind::Rtu => crate::modbus::rtu::encode(UnitId(unit_id), pdu),
        TransportKind::Ascii => crate::modbus::ascii::encode(UnitId(unit_id), pdu),
        TransportKind::Tcp => {
            if engine.tcp_framing == TcpFraming::Rtu {
                crate::modbus::rtu::encode(UnitId(unit_id), pdu)
            } else {
                mbap_frame(unit_id, pdu, tx)
            }
        }
        TransportKind::Udp => {
            if engine.udp_mbap {
                mbap_frame(unit_id, pdu, tx)
            } else {
                crate::modbus::rtu::encode(UnitId(unit_id), pdu)
            }
        }
    }
}

/// Decode a raw RTU ADU and verify the unit id.
fn verify_rtu(adu: &[u8], expected_unit: u8) -> Result<Pdu, TransportError> {
    let frame = crate::modbus::rtu::decode(adu).map_err(TransportError::Modbus)?;
    if frame.unit_id.0 != expected_unit {
        return Err(TransportError::Modbus(ModbusError::UnitIdMismatch {
            expected: expected_unit,
            actual: frame.unit_id.0,
        }));
    }
    Ok(frame.pdu)
}

/// Verify a response ADU against the request (including transaction id for MBAP).
fn verify_adu(
    kind: TransportKind,
    expected_unit: u8,
    expected_fc: u8,
    adu: &[u8],
    expected_tx: u16,
    engine: EngineParams,
) -> Result<Pdu, TransportError> {
    let pdu = match kind {
        TransportKind::Rtu => verify_rtu(adu, expected_unit)?,
        TransportKind::Ascii => {
            let f = crate::modbus::ascii::decode(adu).map_err(TransportError::Modbus)?;
            if f.unit_id.0 != expected_unit {
                return Err(TransportError::Modbus(ModbusError::UnitIdMismatch {
                    expected: expected_unit,
                    actual: f.unit_id.0,
                }));
            }
            f.pdu
        }
        TransportKind::Tcp => {
            if engine.tcp_framing == TcpFraming::Rtu {
                verify_rtu(adu, expected_unit)?
            } else {
                verify_mbap(adu, expected_unit, expected_tx)?
            }
        }
        TransportKind::Udp => {
            if engine.udp_mbap {
                verify_mbap(adu, expected_unit, expected_tx)?
            } else {
                verify_rtu(adu, expected_unit)?
            }
        }
    };

    if pdu.is_exception() {
        return Ok(pdu);
    }
    let fc = pdu.function();
    if fc != expected_fc {
        return Err(TransportError::Modbus(ModbusError::UnexpectedFunction(fc)));
    }
    Ok(pdu)
}

#[cfg(test)]
mod mbap_tests {
    use super::*;

    #[test]
    fn validates_transaction_id() {
        let pdu = [0x03, 0x02, 0x00, 0x2A];
        let frame = mbap_frame(1, &pdu, 0x1234);
        assert!(verify_mbap(&frame, 1, 0x1234).is_ok());
        assert!(matches!(
            verify_mbap(&frame, 1, 0x4321),
            Err(TransportError::Modbus(ModbusError::TransactionIdMismatch { .. }))
        ));
    }

    #[test]
    fn rejects_bad_length() {
        let mut frame = mbap_frame(1, &[0x03, 0x00], 7);
        frame[5] = 99;
        assert!(verify_mbap(&frame, 1, 7).is_err());
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(unused_assignments)]
async fn execute_probe(
    app: &tauri::AppHandle,
    serial_transport: &mut Option<SerialTransport>,
    target: &DeviceTarget,
    function: u8,
    pdu: &[u8],
    results: &Arc<RwLock<ScanResults>>,
    engine: EngineParams,
) -> ProbeOutcome {
    results.write().stats.requests_sent += 1;
    let transaction_id = next_transaction_id();
    let adu = build_adu(target.kind, target.unit_id, pdu, engine, transaction_id);

    let mut attempts = 0u32;
    let retries = engine.retries;
    let timeout = engine.timeout;
    let mut last_err: Option<TransportError> = None;

    loop {
        let started = Instant::now();
        // P2-04: did the TCP handshake succeed? A connected socket that never
        // speaks Modbus is different from a refused/unreachable endpoint.
        let mut tcp_connected = false;
        let res: Result<Vec<u8>, TransportError> = match target.kind {
            TransportKind::Rtu | TransportKind::Ascii => {
                let mode = match target.serial_mode {
                    Some(SerialMode::Ascii) => SerialMode::Ascii,
                    _ => SerialMode::Rtu,
                };
                if serial_transport.is_none() {
                    let sp = match target.serial.clone() {
                        Some(sp) => sp,
                        None => {
                            last_err = Some(TransportError::Other("serial params required".into()));
                            break;
                        }
                    };
                    let cfg = SerialPortConfig {
                        port_name: sp.port_name,
                        baud_rate: sp.baud_rate,
                        data_bits: sp.data_bits.into(),
                        parity: sp.parity.into(),
                        stop_bits: sp.stop_bits.into(),
                        frame_gap: Duration::from_millis(2),
                    };
                    match SerialTransport::open(cfg, mode).await {
                        Ok(t) => *serial_transport = Some(t),
                        Err(e) => {
                            let mut r = results.write();
                            r.stats.transport_errors += 1;
                            last_err = Some(e);
                            break;
                        }
                    }
                }
                let t = serial_transport.as_mut().unwrap();
                match t.transact(&adu, timeout, 8).await {
                    Ok(resp) => Ok(resp),
                    Err(e) => {
                        if matches!(e, TransportError::Timeout(_)) {
                            // after a timeout, reset the RX buffer
                            let _ = t.reset().await;
                        }
                        Err(e)
                    }
                }
            }
            TransportKind::Tcp => match TcpTransport::connect(crate::transport::tcp::TcpConfig {
                host: target.endpoint.clone(),
                port: target.port.unwrap_or(502),
                connect_timeout: timeout,
                framing: engine.tcp_framing,
            })
            .await
            {
                Ok(mut t) => {
                    tcp_connected = true;
                    match t.transact(&adu, timeout, 8).await {
                        Ok(resp) => {
                            let _ = t.close().await;
                            Ok(resp)
                        }
                        Err(e) => Err(e),
                    }
                }
                Err(e) => Err(e),
            },
            TransportKind::Udp => match UdpTransport::connect(&target.endpoint, target.port.unwrap_or(502)).await {
                Ok(mut t) => t.transact(&adu, timeout, 8).await,
                Err(e) => Err(e),
            },
        };

        match res {
            Ok(resp) => {
                let rtt = started.elapsed().as_micros();
                match verify_adu(target.kind, target.unit_id, function, &resp, transaction_id, engine) {
                    Ok(p) => {
                        let result = super::strategies::classify_response(function, &p.to_bytes(), rtt);
                        let is_found = matches!(result, ProbeResult::Ok { .. } | ProbeResult::Exception { .. });
                        let is_ok = matches!(result, ProbeResult::Ok { .. });
                        let is_exception = matches!(result, ProbeResult::Exception { .. });
                        if is_found {
                            note_ip_status(
                                app,
                                results,
                                target,
                                IpTargetClass::ValidModbus,
                                is_ok,
                                is_exception,
                                false,
                            );
                        } else {
                            // Connected, answered, but not valid Modbus data.
                            let class = if target.kind == TransportKind::Tcp {
                                IpTargetClass::PortOpenNoModbus
                            } else {
                                IpTargetClass::Timeout
                            };
                            note_ip_status(app, results, target, class, false, false, false);
                        }

                        let key = format!(
                            "{}:{}:{}",
                            kind_short(target.kind),
                            target.endpoint,
                            target.port.map(|p| p.to_string()).unwrap_or_default()
                        );
                        let key = format!("{key}:{}", target.unit_id);

                        // Update results inside a closed scope: the write guard
                        // must not live across the await below.
                        let record: Option<DeviceRecord> = {
                            let mut r = results.write();
                            match &result {
                                ProbeResult::Ok { .. } => r.stats.responses_ok += 1,
                                ProbeResult::Exception { .. } => r.stats.responses_exception += 1,
                                _ => {}
                            }
                            if is_found {
                                if let Some(rec) = r.records.iter_mut().find(|rec| rec.key == key) {
                                    rec.probes.insert(function, result.clone());
                                    rec.last_seen_ms = now_ms();
                                } else {
                                    let mut probes = HashMap::new();
                                    probes.insert(function, result.clone());
                                    r.records.push(DeviceRecord {
                                        key: key.clone(),
                                        target: target.clone(),
                                        probes,
                                        registers: HashMap::new(),
                                        device_identification: None,
                                        first_seen_ms: now_ms(),
                                        last_seen_ms: now_ms(),
                                    });
                                }
                                r.records.iter().find(|rec| rec.key == key).cloned()
                            } else {
                                None
                            }
                        };

                        if is_found {
                            if let Some(rec) = record {
                                emit_event(app, &ScanEvent::DeviceFound { record: rec });
                            }
                            // P2-05: ask the device who it is (best-effort).
                            identify_device(app, serial_transport, target, results, &key, engine).await;
                            // The register plan is scheduled by the engine (it owns
                            // the strategy and the shared serial transport).
                            return ProbeOutcome {
                                is_found: true,
                                target: target.clone(),
                            };
                        }
                        return ProbeOutcome {
                            is_found: false,
                            target: target.clone(),
                        };
                    }
                    Err(e) => {
                        let class = if target.kind == TransportKind::Tcp {
                            IpTargetClass::PortOpenNoModbus
                        } else {
                            IpTargetClass::Timeout
                        };
                        note_ip_status(app, results, target, class, false, false, true);
                        last_err = Some(e);
                    }
                }
            }
            Err(e) => {
                let timed_out = matches!(e, TransportError::Timeout(_));
                let class = if target.kind == TransportKind::Tcp {
                    if tcp_connected {
                        IpTargetClass::PortOpenNoModbus
                    } else {
                        classify_ip_error(&e)
                    }
                } else {
                    IpTargetClass::Timeout
                };
                note_ip_status(app, results, target, class, false, false, timed_out);
                last_err = Some(e);
            }
        }

        attempts += 1;
        if attempts > retries {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // No response / error after retries.
    {
        let mut r = results.write();
        match &last_err {
            Some(TransportError::Timeout(_)) | None => r.stats.timeouts += 1,
            Some(_) => r.stats.transport_errors += 1,
        }
    }
    ProbeOutcome {
        is_found: false,
        target: target.clone(),
    }
}

async fn execute_work(
    app: &tauri::AppHandle,
    item: &WorkItem,
    serial_transport: &mut Option<SerialTransport>,
    results: &Arc<RwLock<ScanResults>>,
    engine: EngineParams,
) -> Option<RegisterReadOutcome> {
    if let WorkItem::RegisterRead { target, function, start, quantity, pdu } = item {
        results.write().stats.requests_sent += 1;
        Some(
            execute_register_read(
                app,
                serial_transport,
                target,
                *function,
                *start,
                *quantity,
                pdu,
                results,
                engine,
            )
            .await,
        )
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_register_read(
    app: &tauri::AppHandle,
    serial_transport: &mut Option<SerialTransport>,
    target: &DeviceTarget,
    function: u8,
    start: u16,
    quantity: u16,
    pdu: &[u8],
    results: &Arc<RwLock<ScanResults>>,
    engine: EngineParams,
) -> RegisterReadOutcome {
    let transaction_id = next_transaction_id();
    let adu = build_adu(target.kind, target.unit_id, pdu, engine, transaction_id);
    let timeout = engine.timeout;
    let started = Instant::now();

    let res: Result<Vec<u8>, TransportError> = match target.kind {
        TransportKind::Rtu | TransportKind::Ascii => {
            if serial_transport.is_none() {
                return RegisterReadOutcome::Error;
            }
            let t = serial_transport.as_mut().unwrap();
            t.transact(&adu, timeout, 3 + (quantity as usize) * 2).await
        }
        TransportKind::Tcp => match TcpTransport::connect(crate::transport::tcp::TcpConfig {
            host: target.endpoint.clone(),
            port: target.port.unwrap_or(502),
            connect_timeout: timeout,
            framing: engine.tcp_framing,
        })
        .await
        {
            Ok(mut t) => match t.transact(&adu, timeout, 8).await {
                Ok(resp) => {
                    let _ = t.close().await;
                    Ok(resp)
                }
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        },
        TransportKind::Udp => match UdpTransport::connect(&target.endpoint, target.port.unwrap_or(502)).await {
            Ok(mut t) => t.transact(&adu, timeout, 8).await,
            Err(e) => Err(e),
        },
    };

    let resp = match res {
        Ok(r) => r,
        Err(e) => {
            let timed_out = matches!(e, TransportError::Timeout(_));
            let mut r = results.write();
            if timed_out {
                r.stats.timeouts += 1;
            } else {
                r.stats.transport_errors += 1;
            }
            return if timed_out {
                RegisterReadOutcome::Timeout
            } else {
                RegisterReadOutcome::Error
            };
        }
    };

    let rtt = started.elapsed().as_micros();
    match verify_adu(
        target.kind,
        target.unit_id,
        function,
        &resp,
        transaction_id,
        engine,
    ) {
        Ok(p) if !p.is_exception() => {
            let data = match &p {
                Pdu::Response { data, .. } => data.clone(),
                _ => Vec::new(),
            };
            let key = format!(
                "{}:{}:{}",
                kind_short(target.kind),
                target.endpoint,
                target.port.map(|p| p.to_string()).unwrap_or_default()
            );
            let key = format!("{key}:{}", target.unit_id);

            let values: Vec<u16> = match function {
                0x03 | 0x04 => crate::modbus::frame::response::parse_registers(&data).unwrap_or_default(),
                0x01 | 0x02 => crate::modbus::frame::response::parse_bits(&data, quantity)
                    .map(|bits| bits.iter().map(|&b| b as u16).collect())
                    .unwrap_or_default(),
                _ => Vec::new(),
            };

            let mut r = results.write();
            r.stats.responses_ok += 1;
            // A register-map scan can target a device that was never discovered;
            // create the record on demand so its values are not dropped.
            let idx = match r.records.iter().position(|rec| rec.key == key) {
                Some(i) => i,
                None => {
                    r.records.push(DeviceRecord {
                        key: key.clone(),
                        target: target.clone(),
                        probes: HashMap::new(),
                        registers: HashMap::new(),
                        device_identification: None,
                        first_seen_ms: now_ms(),
                        last_seen_ms: now_ms(),
                    });
                    r.records.len() - 1
                }
            };
            let rec = &mut r.records[idx];
            for (i, raw) in values.iter().enumerate() {
                rec.registers.insert(
                    format!("{function:02x}:{:04x}", start + i as u16),
                    RegisterValue {
                        function,
                        address: start + i as u16,
                        raw: *raw,
                        as_hex: format!("{raw:04x}"),
                        rtt_us: rtt,
                    },
                );
            }
            rec.last_seen_ms = now_ms();
            let rec_clone = rec.clone();
            drop(r);
            emit_event(app, &ScanEvent::DeviceUpdated { record: rec_clone });
            RegisterReadOutcome::Ok {
                values: values.clone(),
            }
        }
        Ok(p) => {
            let code = match p {
                Pdu::Exception { code, .. } => code,
                _ => 0x02,
            };
            let mut r = results.write();
            r.stats.responses_exception += 1;
            RegisterReadOutcome::Exception { code }
        }
        Err(e) => {
            let mut r = results.write();
            r.stats.transport_errors += 1;
            let _ = e;
            RegisterReadOutcome::Error
        }
    }
}

/// Classify a transport failure observed while connecting to an IP target.
fn classify_ip_error(err: &TransportError) -> IpTargetClass {
    match err {
        TransportError::Timeout(_) => IpTargetClass::Timeout,
        TransportError::Io(io) => match io.kind() {
            std::io::ErrorKind::ConnectionRefused => IpTargetClass::ConnectionRefused,
            std::io::ErrorKind::HostUnreachable | std::io::ErrorKind::NetworkUnreachable => {
                IpTargetClass::Unreachable
            }
            std::io::ErrorKind::TimedOut => IpTargetClass::Timeout,
            _ => IpTargetClass::Timeout,
        },
        _ => IpTargetClass::Timeout,
    }
}

/// Record and (on change) emit the classification of one IP endpoint.
#[allow(clippy::too_many_arguments)]
fn note_ip_status(
    app: &tauri::AppHandle,
    results: &Arc<RwLock<ScanResults>>,
    target: &DeviceTarget,
    class: IpTargetClass,
    ok: bool,
    exception: bool,
    timed_out: bool,
) {
    if !matches!(target.kind, TransportKind::Tcp | TransportKind::Udp) {
        return;
    }
    let port = target.port.unwrap_or(502);
    let (changed, snapshot) = {
        let mut r = results.write();
        let idx = match r.ip_targets.iter().position(|s| s.endpoint == target.endpoint && s.port == port) {
            Some(i) => i,
            None => {
                r.ip_targets.push(IpTargetStatus {
                    endpoint: target.endpoint.clone(),
                    port,
                    class: IpTargetClass::Unknown,
                    unit_ids_probed: Vec::new(),
                    responses_ok: 0,
                    responses_exception: 0,
                    timeouts: 0,
                    transport_errors: 0,
                    last_seen_ms: now_ms(),
                });
                r.ip_targets.len() - 1
            }
        };
        let status = &mut r.ip_targets[idx];
        let previous = status.class;
        if class.rank() > status.class.rank() {
            status.class = class;
        }
        if !status.unit_ids_probed.contains(&target.unit_id) {
            status.unit_ids_probed.push(target.unit_id);
        }
        if ok {
            status.responses_ok += 1;
        }
        if exception {
            status.responses_exception += 1;
        }
        if timed_out {
            status.timeouts += 1;
        } else if !ok && !exception {
            status.transport_errors += 1;
        }
        status.last_seen_ms = now_ms();
        (previous != status.class, status.clone())
    };
    if changed {
        emit_event(app, &ScanEvent::IpTargetClassified { status: snapshot });
    }
}

/// Try FC43/MEI14 identification once a device is known to answer.
async fn identify_device(
    app: &tauri::AppHandle,
    serial_transport: &mut Option<SerialTransport>,
    target: &DeviceTarget,
    results: &Arc<RwLock<ScanResults>>,
    key: &str,
    engine: EngineParams,
) {
    if !engine.identify_devices {
        return;
    }
    let transaction_id = next_transaction_id();
    let adu = build_adu(
        target.kind,
        target.unit_id,
        &read_device_identification(1, 0),
        engine,
        transaction_id,
    );
    let timeout = engine.timeout;
    let res: Result<Vec<u8>, TransportError> = match target.kind {
        TransportKind::Rtu | TransportKind::Ascii => match serial_transport.as_mut() {
            Some(t) => t.transact(&adu, timeout, 32).await,
            None => return,
        },
        TransportKind::Tcp => match TcpTransport::connect(crate::transport::tcp::TcpConfig {
            host: target.endpoint.clone(),
            port: target.port.unwrap_or(502),
            connect_timeout: timeout,
            framing: engine.tcp_framing,
        })
        .await
        {
            Ok(mut t) => {
                let out = t.transact(&adu, timeout, 32).await;
                let _ = t.close().await;
                out
            }
            Err(e) => Err(e),
        },
        TransportKind::Udp => match UdpTransport::connect(&target.endpoint, target.port.unwrap_or(502)).await {
            Ok(mut t) => t.transact(&adu, timeout, 32).await,
            Err(e) => Err(e),
        },
    };
    let resp = match res {
        Ok(r) => r,
        Err(_) => return,
    };
    let pdu = match verify_adu(target.kind, target.unit_id, 0x2B, &resp, transaction_id, engine) {
        Ok(p) if !p.is_exception() => p,
        _ => return,
    };
    let data = match &pdu {
        Pdu::Response { data, .. } => data.clone(),
        _ => return,
    };
    let identity = match parse_device_identification(&data) {
        Ok(id) => id,
        Err(_) => return,
    };
    let record = {
        let mut r = results.write();
        if let Some(rec) = r.records.iter_mut().find(|rec| rec.key == key) {
            rec.device_identification = Some(identity);
            Some(rec.clone())
        } else {
            None
        }
    };
    if let Some(record) = record {
        emit_event(app, &ScanEvent::DeviceUpdated { record });
    }
}

/// Settings currently required by a serial work item (None for IP targets).
fn serial_signature(target: &DeviceTarget) -> Option<String> {
    let sp = target.serial.as_ref()?;
    Some(format!(
        "{}|{}|{:?}|{:?}|{:?}|{:?}",
        sp.port_name,
        sp.baud_rate,
        target.serial_mode,
        sp.data_bits,
        sp.parity,
        sp.stop_bits
    ))
}

/// Reopen the shared serial port when the sweep moves to different settings.
async fn ensure_serial_settings(
    transport: &mut Option<SerialTransport>,
    signature: &mut Option<String>,
    target: &DeviceTarget,
) {
    let next = serial_signature(target);
    if next != *signature {
        if let Some(mut old) = transport.take() {
            let _ = old.close().await;
        }
        *signature = next;
    }
}

fn kind_short(kind: TransportKind) -> &'static str {
    match kind {
        TransportKind::Rtu => "rtu",
        TransportKind::Ascii => "ascii",
        TransportKind::Tcp => "tcp",
        TransportKind::Udp => "udp",
    }
}

fn emit_event(app: &tauri::AppHandle, event: &ScanEvent) {
    use tauri::Emitter;
    if let Err(e) = app.emit("scan:event", event) {
        tracing::warn!("failed to emit scan event: {e}");
    }
}

fn emit_progress(
    app: &tauri::AppHandle,
    status: ScanStatus,
    total: u64,
    done: u64,
    results: &Arc<RwLock<ScanResults>>,
) {
    use tauri::Emitter;
    let r = results.read();
    let progress = ScanProgress {
        status,
        total,
        done,
        found: r.records.len() as u64,
        current: String::new(),
        errors: r.stats.timeouts + r.stats.transport_errors,
        elapsed_ms: r.stats.elapsed_ms(),
    };
    let _ = app.emit("scan:progress", &progress);
}

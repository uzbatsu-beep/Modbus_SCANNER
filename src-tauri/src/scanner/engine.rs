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
use crate::communication::service::CommunicationService;
use crate::transport::connection::SerialLease;
use tokio::sync::{mpsc, watch, Mutex};

use crate::modbus::common::{ModbusError, SerialMode, TransportKind, UnitId};
use crate::modbus::frame::{
    parse_device_identification, parse_tcp_response, read_device_identification,
    validate_response_function, Pdu,
};
use crate::transport::serial::{SerialPortConfig, SerialTransport};
use crate::transport::tcp::{TcpFraming, TcpTransport};
use crate::transport::tls::TlsParams;
use crate::transport::udp::UdpTransport;
use crate::transport::{Transport, TransportError};

use super::strategies::{build_strategy, RegisterReadOutcome, ScanStrategy, WorkItem};
use super::EventSink;
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
#[derive(Debug, Clone)]
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
    /// Modbus Security TLS options.
    pub tls: Option<TlsParams>,
    quiet_gate: Option<Arc<QuietGate>>,
    /// Second independent limiter for TCP connection attempts, including
    /// refused/unreachable endpoints that never get to transmit a PDU.
    connect_gate: Option<Arc<QuietGate>>,
}

/// Shared for all worker tasks of a single scan, including retry and FC43.
#[derive(Debug)]
struct QuietGate { next: Mutex<tokio::time::Instant> }
impl QuietGate {
    fn new() -> Self { Self { next: Mutex::new(tokio::time::Instant::now()) } }
    async fn wait(&self) {
        let mut next = self.next.lock().await;
        let now = tokio::time::Instant::now();
        if *next > now { tokio::time::sleep_until(*next).await; }
        *next = tokio::time::Instant::now() + Duration::from_millis(500);
    }
}

impl EngineParams {
    async fn before_tx(&self) {
        if let Some(gate) = &self.quiet_gate { gate.wait().await; }
    }
    async fn before_ip_connect(&self) {
        if let Some(gate) = &self.connect_gate { gate.wait().await; }
    }
    pub fn from_scan_params(p: &ScanParams) -> Self {
        Self {
            timeout: Duration::from_millis(p.timeout_ms.clamp(50, 10_000) as u64),
            retries: p.retries.min(5),
            request_gap: Duration::from_millis(p.request_gap_ms.min(1_000) as u64),
            // OT Quiet Mode must never open dozens of simultaneous TCP sockets.
            ip_concurrency: if p.quiet_mode { 1 } else { (p.ip_concurrency.clamp(1, 256)) as usize },
            identify_devices: p.identify_devices,
            tcp_framing: p.tcp_framing,
            udp_mbap: p.udp_mbap,
            tls: p.tls.clone(),
            quiet_gate: p.quiet_mode.then(|| Arc::new(QuietGate::new())),
            connect_gate: p.quiet_mode.then(|| Arc::new(QuietGate::new())),
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
    fn quiet_mode_forces_one_ip_worker_and_both_connection_and_tx_gates() {
        let mut params = ScanParams::default();
        params.quiet_mode = true;
        params.ip_concurrency = 128;
        let engine = EngineParams::from_scan_params(&params);
        assert_eq!(engine.ip_concurrency, 1);
        assert!(engine.quiet_gate.is_some());
        assert!(engine.connect_gate.is_some());
        params.quiet_mode = false;
        let normal = EngineParams::from_scan_params(&params);
        assert_eq!(normal.ip_concurrency, 128);
        assert!(normal.quiet_gate.is_none());
        assert!(normal.connect_gate.is_none());
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

/// Independent stop signal must never be queued behind Pause/Resume messages.
#[derive(Clone)]
struct ScanControl {
    commands: mpsc::Sender<ControlMsg>,
    cancel: watch::Sender<bool>,
}

/// Shared engine state handle.
pub struct ScannerHandle {
    communication: Arc<CommunicationService>,
    /// Lifecycle control for the active task (None when idle).
    control_tx: Mutex<Option<ScanControl>>,
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
    pub fn new() -> Self {Self::with_communication(Arc::new(CommunicationService::default()))}

    pub fn with_communication(communication:Arc<CommunicationService>)->Self {
        let (status_tx, _) = watch::channel(ScanStatus::Idle);
        Self {
            communication,
            control_tx: Mutex::new(None),
            status_tx,
            results: Arc::new(RwLock::new(ScanResults::default())),
        }
    }

    pub fn communication(&self)->Arc<CommunicationService>{self.communication.clone()}

    pub fn status(&self) -> ScanStatus {
        *self.status_tx.borrow()
    }

    /// Latest progress snapshot.
    pub fn progress(&self) -> ScanProgress {
        let results = self.results.read();
        let total = results.stats.requests_sent;
        let done = (results.stats.responses_ok
            + results.stats.responses_exception
            + results.stats.timeouts
            + results.stats.transport_errors)
            .min(total);
        ScanProgress {
            status: *self.status_tx.borrow(),
            total,
            done,
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

    /// Replace results from a loaded project (P3-04).
    pub fn set_results(&self, results: ScanResults) {
        *self.results.write() = results;
    }

    /// Wait until the current scan reaches a terminal state (used by the CLI).
    pub async fn wait_finished(&self, timeout: Duration) -> Result<ScanResults, String> {
        let mut rx = self.status_tx.subscribe();
        let deadline = Instant::now() + timeout;
        loop {
            {
                let status = *rx.borrow_and_update();
                if matches!(status, ScanStatus::Finished | ScanStatus::Stopped | ScanStatus::Failed) {
                    return Ok(self.results());
                }
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("scan timeout exceeded".into());
            }
            tokio::time::timeout(remaining, rx.changed())
                .await
                .map_err(|_| "scan timeout exceeded".to_string())?
                .map_err(|e| e.to_string())?;
        }
    }

    /// Start a new scan. Active runs (including paused and preparing) are exclusive.
    pub async fn start(
        &self,
        sink: EventSink,
        target: ScanTarget,
        params: ScanParams,
    ) -> Result<(), String> {
        // Keep preparation and installation atomic with respect to concurrent starts.
        let mut guard = self.control_tx.lock().await;
        if matches!(
            *self.status_tx.borrow(),
            ScanStatus::Preparing | ScanStatus::Running | ScanStatus::Paused | ScanStatus::Stopping
        ) || guard.as_ref().is_some_and(|run| !run.commands.is_closed()) {
            return Err("scan already in progress".into());
        }

        let engine_params = EngineParams::from_scan_params(&params);
        let serial_port=scan_serial_port(&target).map(str::to_owned);
        let strategy = build_strategy(target, params).map_err(|error| {
            self.status_tx.send_replace(ScanStatus::Failed);
            error
        })?;

        let lease=serial_port.as_deref().map(|port|self.communication.manager.lease_serial(port)).transpose().map_err(|e|e.to_string())?;
        let (tx, rx) = mpsc::channel(8);
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let (pause_tx, pause_rx) = watch::channel(false);
        *guard = Some(ScanControl {
            commands: tx,
            cancel: cancel_tx,
        });

        *self.results.write() = ScanResults {
            records: Vec::new(),
            ip_targets: Vec::new(),
            stats: ScanStats {
                started_ms: Some(now_ms()),
                ..Default::default()
            },
        };
        self.status_tx.send_replace(ScanStatus::Preparing);

        tokio::spawn(engine_task(
            sink,
            strategy,
            engine_params,
            rx,
            cancel_rx,
            (pause_tx, pause_rx),
            self.results.clone(),
            self.status_tx.clone(),
            lease,
        ));
        Ok(())
    }

    pub async fn control(&self, msg: ControlMsg) -> Result<(), String> {
        // Do not hold the lifecycle mutex while waiting on the command queue.
        // In particular Stop must always bypass a full Pause/Resume queue.
        let active = self.control_tx.lock().await.clone()
            .ok_or_else(|| "no scan running".to_string())?;
        if active.commands.is_closed() {
            return Err("no scan running".into());
        }

        match msg {
            ControlMsg::Stop => {
                // Only the engine acknowledges Stopping. Publishing it here
                // would race a concurrently finished task and could overwrite
                // a terminal state with a permanently stale "stopping".
                active.cancel.send_replace(true);
                Ok(())
            }
            ControlMsg::Pause | ControlMsg::Resume => {
                if *active.cancel.borrow() {
                    return Err("scan is stopping".into());
                }
                active.commands.send(msg).await.map_err(|e| e.to_string())
            }
        }
    }
}

fn scan_serial_port(target:&ScanTarget)->Option<&str>{
    match target{
        ScanTarget::UnitIdRange{port,..}=>Some(&port.port_name),
        ScanTarget::SerialAuto{sweep}=>Some(&sweep.port_name),
        ScanTarget::RegisterScan{scan}=>device_serial_port(&scan.target),
        ScanTarget::SingleDevice{target,..}=>device_serial_port(target),
        ScanTarget::IpRange{..}=>None,
    }
}
pub(crate) fn device_serial_port(target:&DeviceTarget)->Option<&str>{
    if matches!(target.kind,TransportKind::Rtu|TransportKind::Ascii){target.serial.as_ref().map(|s|s.port_name.as_str())}else{None}
}

/// Wait for a stop request independently of the normal control-message queue.
/// The watch value is sticky, so a Stop issued before a worker starts is not lost.
async fn wait_for_cancel(receiver: &mut watch::Receiver<bool>) {
    loop {
        if *receiver.borrow() {
            return;
        }
        if receiver.changed().await.is_err() {
            return;
        }
    }
}

/// The engine task. User Stop is a hard boundary: cancel in-flight operations,
/// discard follow-up reads and publish Stopped only after all tasks have exited.
async fn engine_task(
    sink: EventSink,
    mut strategy: Box<dyn ScanStrategy + Send>,
    engine: EngineParams,
    mut control_rx: mpsc::Receiver<ControlMsg>,
    mut cancel_rx: watch::Receiver<bool>,
    (pause_tx, mut pause_rx): (watch::Sender<bool>, watch::Receiver<bool>),
    results: Arc<RwLock<ScanResults>>,
    status_tx: watch::Sender<ScanStatus>,
    serial_lease: Option<SerialLease>,
) {
    let total = strategy.total();
    let request_gap = engine.request_gap;
    let ip_concurrency = engine.ip_concurrency;
    let mut done: u64 = 0;
    let mut serial_transport: Option<SerialTransport> = None;
    let mut serial_signature: Option<String> = None;
    let mut pending: std::collections::VecDeque<WorkItem> = std::collections::VecDeque::new();
    // A canceled worker returns None; all other outcomes are normal probe results.
    let mut ip_tasks: tokio::task::JoinSet<Option<ProbeOutcome>> = tokio::task::JoinSet::new();
    let mut stop_on_first = false;
    let mut user_cancelled = *cancel_rx.borrow();

    if !user_cancelled {
        status_tx.send_replace(ScanStatus::Running);
        emit_event(&sink, &ScanEvent::StatusChanged {
            status: ScanStatus::Running,
            message: "scanning started".into(),
        });
    }

    'main: loop {
        if *cancel_rx.borrow() {
            user_cancelled = true;
            break;
        }

        // Pause/Resume are advisory. Stop is also supported here for internal
        // callers, but ScannerHandle::control(Stop) sets the cancel watch directly.
        while let Ok(msg) = control_rx.try_recv() {
            match msg {
                ControlMsg::Stop => {
                    user_cancelled = true;
                    break 'main;
                }
                ControlMsg::Pause => {
                    pause_tx.send_replace(true);
                    status_tx.send_replace(ScanStatus::Paused);
                    emit_event(&sink, &ScanEvent::StatusChanged {
                        status: ScanStatus::Paused,
                        message: "scanning paused".into(),
                    });
                }
                ControlMsg::Resume => {
                    pause_tx.send_replace(false);
                    status_tx.send_replace(ScanStatus::Running);
                    emit_event(&sink, &ScanEvent::StatusChanged {
                        status: ScanStatus::Running,
                        message: "scanning resumed".into(),
                    });
                }
            }
        }

        if *cancel_rx.borrow() {
            user_cancelled = true;
            break;
        }

        while let Some(finished) = ip_tasks.try_join_next() {
            match finished {
                Ok(Some(outcome)) => {
                    let stop_on_found = strategy.stop_on_found();
                    apply_probe_outcome(
                        &mut strategy, &mut pending, outcome, &mut stop_on_first, stop_on_found,
                    );
                    if stop_on_first {
                        break;
                    }
                }
                Err(e) if !e.is_cancelled() => tracing::warn!("IP probe task failed: {e}"),
                _ => {}
            }
        }
        if stop_on_first {
            break;
        }

        if *pause_rx.borrow() {
            tokio::select! {
                biased;
                _ = wait_for_cancel(&mut cancel_rx) => {
                    user_cancelled = true;
                    break;
                }
                changed = pause_rx.changed() => {
                    if changed.is_err() {
                        user_cancelled = true;
                        break;
                    }
                }
                msg = control_rx.recv() => {
                    match msg {
                        Some(ControlMsg::Stop) | None => {
                            user_cancelled = true;
                            break;
                        }
                        Some(ControlMsg::Resume) => {
                            pause_tx.send_replace(false);
                            status_tx.send_replace(ScanStatus::Running);
                            emit_event(&sink, &ScanEvent::StatusChanged {
                                status: ScanStatus::Running,
                                message: "scanning resumed".into(),
                            });
                        }
                        Some(ControlMsg::Pause) => {}
                    }
                }
            }
            continue;
        }

        let item = match pending.pop_front() {
            Some(item) => item,
            None => match strategy.next_item() {
                Some(item) => item,
                None => break,
            },
        };

        match item {
            WorkItem::Probe { target, function, pdu } => {
                if matches!(target.kind, TransportKind::Tcp | TransportKind::Udp) {
                    while ip_tasks.len() >= ip_concurrency {
                        let finished = tokio::select! {
                            biased;
                            _ = wait_for_cancel(&mut cancel_rx) => {
                                user_cancelled = true;
                                break 'main;
                            }
                            outcome = ip_tasks.join_next() => outcome,
                        };
                        match finished {
                            Some(Ok(Some(outcome))) => {
                                let stop_on_found = strategy.stop_on_found();
                                apply_probe_outcome(
                                    &mut strategy, &mut pending, outcome,
                                    &mut stop_on_first, stop_on_found,
                                );
                            }
                            Some(Err(e)) if !e.is_cancelled() => {
                                tracing::warn!("IP probe task failed: {e}");
                            }
                            _ => {}
                        }
                        if stop_on_first {
                            break 'main;
                        }
                    }
                    if *cancel_rx.borrow() {
                        user_cancelled = true;
                        break;
                    }

                    let task_sink = sink.clone();
                    let task_results = results.clone();
                    let task_engine = engine.clone();
                    let mut task_cancel = cancel_rx.clone();
                    ip_tasks.spawn(async move {
                        let mut unused_serial = None;
                        tokio::select! {
                            biased;
                            _ = wait_for_cancel(&mut task_cancel) => None,
                            outcome = execute_probe(
                                &task_sink, &mut unused_serial, &target,
                                function, &pdu, &task_results, task_engine,
                            ) => Some(outcome),
                        }
                    });
                } else {
                    // Cancellation also interrupts serial port setup and blocking RX.
                    let outcome = tokio::select! {
                        biased;
                        _ = wait_for_cancel(&mut cancel_rx) => {
                            user_cancelled = true;
                            break 'main;
                        }
                        value = async {
                            ensure_serial_settings(
                                &mut serial_transport, &mut serial_signature, &target,
                            ).await;
                            execute_probe(
                                &sink, &mut serial_transport, &target,
                                function, &pdu, &results, engine.clone(),
                            ).await
                        } => value,
                    };
                    let stop_on_found = strategy.stop_on_found();
                    apply_probe_outcome(
                        &mut strategy, &mut pending, outcome, &mut stop_on_first, stop_on_found,
                    );
                }
                done += 1;
                emit_progress(&sink, *status_tx.borrow(), total, done, &results);
            }
            item @ WorkItem::RegisterRead { .. } => {
                let outcome = tokio::select! {
                    biased;
                    _ = wait_for_cancel(&mut cancel_rx) => {
                        user_cancelled = true;
                        break 'main;
                    }
                    value = async {
                        if let WorkItem::RegisterRead { target, .. } = &item {
                            ensure_serial_settings(
                                &mut serial_transport, &mut serial_signature, target,
                            ).await;
                        }
                        execute_work(
                            &sink, &item, &mut serial_transport, &results, engine.clone(),
                        ).await
                    } => value,
                };
                if let Some(outcome) = outcome {
                    for follow in strategy.on_register_read(&item, outcome) {
                        pending.push_back(follow);
                    }
                }
                done += 1;
                emit_progress(&sink, *status_tx.borrow(), total, done, &results);
            }
        }

        // The user can stop immediately even during a long inter-request gap.
        if !request_gap.is_zero() {
            tokio::select! {
                biased;
                _ = wait_for_cancel(&mut cancel_rx) => {
                    user_cancelled = true;
                    break;
                }
                _ = tokio::time::sleep(request_gap) => {}
            }
        }
    }

    if !user_cancelled {
        // Normal exhaustion or stop-on-first: collect outstanding probes.
        // For stop-on-first no further discovery probes should be sent.
        if stop_on_first {
            ip_tasks.abort_all();
        }
        while !ip_tasks.is_empty() {
            let finished = tokio::select! {
                biased;
                _ = wait_for_cancel(&mut cancel_rx) => {
                    user_cancelled = true;
                    break;
                }
                outcome = ip_tasks.join_next() => outcome,
            };
            if let Some(Ok(Some(outcome))) = finished {
                let stop_on_found = strategy.stop_on_found();
                apply_probe_outcome(
                    &mut strategy, &mut pending, outcome, &mut stop_on_first, stop_on_found,
                );
                if stop_on_first {
                    ip_tasks.abort_all();
                }
            }
        }

        // Only automatic stop-on-first may complete its planned register reads.
        // An explicit user Stop must never drain this queue.
        while !user_cancelled {
            let Some(item) = pending.pop_front() else { break };
            let outcome = tokio::select! {
                biased;
                _ = wait_for_cancel(&mut cancel_rx) => {
                    user_cancelled = true;
                    break;
                }
                value = async {
                    if let WorkItem::RegisterRead { target, .. } = &item {
                        ensure_serial_settings(
                            &mut serial_transport, &mut serial_signature, target,
                        ).await;
                    }
                    execute_work(
                        &sink, &item, &mut serial_transport, &results, engine.clone(),
                    ).await
                } => value,
            };
            if let Some(outcome) = outcome {
                for follow in strategy.on_register_read(&item, outcome) {
                    pending.push_back(follow);
                }
            }
            done += 1;
            emit_progress(&sink, *status_tx.borrow(), total, done, &results);
            if !request_gap.is_zero() {
                tokio::select! {
                    biased;
                    _ = wait_for_cancel(&mut cancel_rx) => {
                        user_cancelled = true;
                        break;
                    }
                    _ = tokio::time::sleep(request_gap) => {}
                }
            }
        }
    }

    if user_cancelled {
        status_tx.send_replace(ScanStatus::Stopping);
        emit_event(&sink, &ScanEvent::StatusChanged {
            status: ScanStatus::Stopping,
            message: "cancelling in-flight requests and discarding queued reads".into(),
        });
        pending.clear();
        ip_tasks.abort_all();
        while ip_tasks.join_next().await.is_some() {}
    }

    // Drop transports and close the control receiver before the terminal
    // watch update. A caller may restart immediately after wait_finished().
    if let Some(mut transport) = serial_transport.take() {
        let _ = transport.close().await;
    }
    drop(serial_lease);
    control_rx.close();
    user_cancelled |= *cancel_rx.borrow();
    {
        let mut current = results.write();
        current.stats.finished_ms = Some(now_ms());
    }
    let status = if user_cancelled || stop_on_first {
        ScanStatus::Stopped
    } else {
        ScanStatus::Finished
    };
    // Publish terminal events before notifying wait_finished() and allowing
    // restart. Otherwise an old scan's final event can overwrite the next
    // scan's UI status after the previous status already looked terminal.
    emit_event(&sink, &ScanEvent::StatusChanged {
        status,
        message: if user_cancelled {
            "scan cancelled by user".into()
        } else if stop_on_first {
            "stopped after first device".into()
        } else {
            "scan completed".into()
        },
    });
    emit_progress(&sink, status, total, done, &results);
    let current = results.read();
    emit_event(&sink, &ScanEvent::Finished {
        total: done,
        found: current.records.len() as u64,
        errors: current.stats.timeouts + current.stats.transport_errors,
        elapsed_ms: current.stats.elapsed_ms(),
    });
    drop(current);
    status_tx.send_replace(status);
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

/// What a completed probe hands back to the engine.
#[derive(Debug)]
struct ProbeOutcome {
    /// True when the device answered (data or exception).
    is_found: bool,
    /// Kept so the engine can schedule the register plan for a found device.
    target: DeviceTarget,
}

/// Next MBAP transaction id (shared by all concurrent IP probes).
pub(crate) fn next_transaction_id() -> u16 {
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
    parse_tcp_response(adu, expected_unit, expected_tx).map_err(TransportError::Modbus)
}

/// Build the wire ADU, honouring the selected TCP/UDP framing.
pub(crate) fn build_adu(kind: TransportKind, unit_id: u8, pdu: &[u8], engine: EngineParams, tx: u16) -> Vec<u8> {
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
pub(crate) fn verify_adu(
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

    validate_response_function(pdu, expected_fc).map_err(TransportError::Modbus)
}

#[cfg(test)]
mod mock_pipeline_tests {
    use super::*;
    use crate::modbus::frame::request;
    use crate::transport::mock::MockTransport;
    use crate::transport::Transport;

    #[tokio::test]
    async fn build_transact_verify_with_mock_transport() {
        let params = ScanParams {
            timeout_ms: 100,
            ..Default::default()
        };
        let engine = EngineParams::from_scan_params(&params);
        let tx = 0x1234u16;
        let adu = build_adu(
            TransportKind::Tcp,
            1,
            &request::read_holding_registers(0, 1),
            engine.clone(),
            tx,
        );
        // Valid MBAP response: FC03, byte count 2, value 0x002A.
        let response = mbap_frame(1, &[0x03, 0x02, 0x00, 0x2A], tx);
        let mut mock = MockTransport::new(vec![Ok(response)]);

        let raw = mock
            .transact(&adu, Duration::from_millis(100), 8)
            .await
            .unwrap();
        let pdu = verify_adu(TransportKind::Tcp, 1, 0x03, &raw, tx, engine).unwrap();
        assert_eq!(pdu.to_bytes(), vec![0x03, 0x02, 0x00, 0x2A]);
        assert_eq!(mock.sent.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn transaction_id_mismatch_is_rejected() {
        let params = ScanParams::default();
        let engine = EngineParams::from_scan_params(&params);
        let response = mbap_frame(1, &[0x03, 0x02, 0x00, 0x2A], 0x9999);
        let err = verify_adu(TransportKind::Tcp, 1, 0x03, &response, 0x1234, engine).unwrap_err();
        assert!(err.to_string().contains("transaction"));
    }
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

    #[test]
    fn strict_exception_fc_on_all_transports() {
        use crate::modbus::{ascii, rtu};
        let engine = EngineParams::from_scan_params(&ScanParams::default());
        for (kind, raw) in [
            (TransportKind::Rtu, rtu::encode(UnitId(1), &[0x84, 2])),
            (TransportKind::Ascii, ascii::encode(UnitId(1), &[0x84, 2])),
            (TransportKind::Udp, rtu::encode(UnitId(1), &[0x84, 2])),
            (TransportKind::Tcp, mbap_frame(1, &[0x84, 2], 10)),
        ] {
            assert!(matches!(
                verify_adu(kind, 1, 3, &raw, 10, engine.clone()),
                Err(TransportError::Modbus(ModbusError::UnexpectedFunction(4)))
            ));
        }
        for (kind, raw) in [
            (TransportKind::Rtu, rtu::encode(UnitId(1), &[0x83, 2, 0])),
            (TransportKind::Ascii, ascii::encode(UnitId(1), &[0x83, 2, 0])),
            (TransportKind::Udp, rtu::encode(UnitId(1), &[0x83, 2, 0])),
            (TransportKind::Tcp, mbap_frame(1, &[0x83, 2, 0], 10)),
        ] {
            assert!(verify_adu(kind, 1, 3, &raw, 10, engine.clone()).is_err());
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(unused_assignments)]
async fn execute_probe(
    sink: &EventSink,
    serial_transport: &mut Option<SerialTransport>,
    target: &DeviceTarget,
    function: u8,
    pdu: &[u8],
    results: &Arc<RwLock<ScanResults>>,
    engine: EngineParams,
) -> ProbeOutcome {
    results.write().stats.requests_sent += 1;
    let transaction_id = next_transaction_id();
    let adu = build_adu(target.kind, target.unit_id, pdu, engine.clone(), transaction_id);

    let mut attempts = 0u32;
    let retries = engine.retries;
    let timeout = engine.timeout;
    let mut last_err: Option<TransportError> = None;

    loop {
        let started = Instant::now();
        // P2-04: did the TCP handshake succeed? A connected socket that never
        // speaks Modbus is different from a refused/unreachable endpoint.
        let mut tcp_connected = false;
        if target.kind == TransportKind::Tcp {
            engine.before_ip_connect().await;
        }
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
                engine.before_tx().await;
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
            tls: engine.tls.clone(),
            })
            .await
            {
                Ok(mut t) => {
                    tcp_connected = true;
                    engine.before_tx().await;
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
                Ok(mut t) => { engine.before_tx().await; t.transact(&adu, timeout, 8).await },
                Err(e) => Err(e),
            },
        };

        // Verify before logging the outcome. Receiving bytes is not itself
        // a successful Modbus transaction: malformed PDU/counts must be marked
        // "invalid" in the traffic log as well as in discovery statistics.
        let verified = res.as_ref().ok().map(|reply| {
            verify_adu(
                target.kind, target.unit_id, function,
                reply, transaction_id, engine.clone(),
            ).map(|p| super::strategies::classify_response(
                function, &p.to_bytes(), started.elapsed().as_micros(),
            ))
        });
        let status = match (&res, &verified) {
            (Ok(_), Some(Ok(ProbeResult::Ok { .. }))) => "ok",
            (Ok(_), Some(Ok(ProbeResult::Exception { .. }))) => "exception",
            (Ok(_), _) => "invalid",
            (Err(TransportError::Timeout(_)), _) => "timeout",
            (Err(_), _) => "error",
        };
        log_traffic(
            sink,
            target,
            Some(function),
            &adu,
            res.as_ref().ok(),
            started.elapsed().as_micros(),
            status,
        );

        match res {
            Ok(_) => {
                let verified = verified.unwrap_or_else(|| {
                    Err(TransportError::Other("missing probe verification".into()))
                });
                match verified {
                    Ok(result) => {
                        let is_found = matches!(result, ProbeResult::Ok { .. } | ProbeResult::Exception { .. });
                        let is_ok = matches!(result, ProbeResult::Ok { .. });
                        let is_exception = matches!(result, ProbeResult::Exception { .. });
                        if is_found {
                            note_ip_status(
                                sink,
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
                            note_ip_status(sink, results, target, class, false, false, false);
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
                                ProbeResult::TransportError { .. } => r.stats.transport_errors += 1,
                                ProbeResult::NoResponse { .. } => r.stats.timeouts += 1,
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
                                emit_event(sink, &ScanEvent::DeviceFound { record: rec });
                            }
                            // P2-05: ask the device who it is (best-effort).
                            identify_device(sink, serial_transport, target, results, &key, engine.clone()).await;
                            // The register plan is scheduled by the engine (it owns
                            // the strategy and the shared serial transport).
                            emit_probe_completed(sink, &key, function, &result);
                            return ProbeOutcome {
                                is_found: true,
                                target: target.clone(),
                            };
                        }
                        emit_probe_completed(sink, &key, function, &result);
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
                        note_ip_status(sink, results, target, class, false, false, true);
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
                note_ip_status(sink, results, target, class, false, false, timed_out);
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
    let key = format!(
        "{}:{}:{}:{}",
        kind_short(target.kind),
        target.endpoint,
        target.port.map(|p| p.to_string()).unwrap_or_default(),
        target.unit_id
    );
    let result = match &last_err {
        Some(TransportError::Timeout(_)) | None => ProbeResult::NoResponse { function },
        Some(e) => ProbeResult::TransportError {
            function,
            message: e.to_string(),
        },
    };
    emit_probe_completed(sink, &key, function, &result);
    ProbeOutcome {
        is_found: false,
        target: target.clone(),
    }
}

async fn execute_work(
    sink: &EventSink,
    item: &WorkItem,
    serial_transport: &mut Option<SerialTransport>,
    results: &Arc<RwLock<ScanResults>>,
    engine: EngineParams,
) -> Option<RegisterReadOutcome> {
    if let WorkItem::RegisterRead { target, function, start, quantity, pdu } = item {
        results.write().stats.requests_sent += 1;
        Some(
            execute_register_read(
                sink,
                serial_transport,
                target,
                *function,
                *start,
                *quantity,
                pdu,
                results,
                engine.clone(),
            )
            .await,
        )
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_register_read(
    sink: &EventSink,
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
    let adu = build_adu(target.kind, target.unit_id, pdu, engine.clone(), transaction_id);
    let timeout = engine.timeout;
    let started = Instant::now();

    if target.kind == TransportKind::Tcp {
        engine.before_ip_connect().await;
    }
    let res: Result<Vec<u8>, TransportError> = match target.kind {
        TransportKind::Rtu | TransportKind::Ascii => {
            if serial_transport.is_none() {
                return RegisterReadOutcome::Error;
            }
            let t = serial_transport.as_mut().unwrap();
            engine.before_tx().await;
            t.transact(&adu, timeout, 3 + (quantity as usize) * 2).await
        }
        TransportKind::Tcp => match TcpTransport::connect(crate::transport::tcp::TcpConfig {
            host: target.endpoint.clone(),
            port: target.port.unwrap_or(502),
            connect_timeout: timeout,
            framing: engine.tcp_framing,
        tls: engine.tls.clone(),
        })
        .await
        {
            Ok(mut t) => { engine.before_tx().await;
                match t.transact(&adu, timeout, 8).await {
                Ok(resp) => {
                    let _ = t.close().await;
                    Ok(resp)
                }
                Err(e) => Err(e),
                }
            },
            Err(e) => Err(e),
        },
        TransportKind::Udp => match UdpTransport::connect(&target.endpoint, target.port.unwrap_or(502)).await {
            Ok(mut t) => { engine.before_tx().await; t.transact(&adu, timeout, 8).await },
            Err(e) => Err(e),
        },
    };

    // Validate once, before both traffic logging and result storage. A reply
    // with valid MBAP but a wrong byte count must not be displayed as "ok".
    let verified: Option<Result<Result<Vec<u16>, u8>, TransportError>> =
        res.as_ref().ok().map(|reply| {
            verify_adu(
                target.kind, target.unit_id, function,
                reply, transaction_id, engine.clone(),
            ).and_then(|pdu| match pdu {
                Pdu::Response { data, .. } => {
                    crate::modbus::frame::response::parse_read_values(
                        function, &data, quantity,
                    ).map(Ok).map_err(TransportError::Modbus)
                }
                Pdu::Exception { code, .. } => Ok(Err(code)),
                _ => Err(TransportError::Modbus(ModbusError::InvalidFrame(
                    "register read did not return a response PDU".into()
                ))),
            })
        });
    let tx_status = match (&res, &verified) {
        (Ok(_), Some(Ok(Ok(_)))) => "ok",
        (Ok(_), Some(Ok(Err(_)))) => "exception",
        (Ok(_), _) => "invalid",
        (Err(TransportError::Timeout(_)), _) => "timeout",
        (Err(_), _) => "error",
    };
    log_traffic(
        sink,
        target,
        Some(function),
        &adu,
        res.as_ref().ok(),
        started.elapsed().as_micros(),
        tx_status,
    );

    if let Err(e) = res {
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

    let rtt = started.elapsed().as_micros();
    let parsed = verified.unwrap_or_else(|| {
        Err(TransportError::Other("missing register read verification".into()))
    });
    match parsed {
        Ok(Ok(values)) => {
            let key = format!(
                "{}:{}:{}",
                kind_short(target.kind),
                target.endpoint,
                target.port.map(|p| p.to_string()).unwrap_or_default()
            );
            let key = format!("{key}:{}", target.unit_id);

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
            emit_event(sink, &ScanEvent::DeviceUpdated { record: rec_clone });
            RegisterReadOutcome::Ok {
                values: values.clone(),
            }
        }
        Ok(Err(code)) => {
            results.write().stats.responses_exception += 1;
            RegisterReadOutcome::Exception { code }
        }
        Err(e) => {
            tracing::warn!("invalid Modbus read response FC {function:#04x}: {e}");
            results.write().stats.transport_errors += 1;
            RegisterReadOutcome::Malformed
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
    sink: &EventSink,
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
        emit_event(sink, &ScanEvent::IpTargetClassified { status: snapshot });
    }
}

/// Try FC43/MEI14 identification once a device is known to answer.
async fn identify_device(
    sink: &EventSink,
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
        engine.clone(),
        transaction_id,
    );
    let timeout = engine.timeout;
    let started = Instant::now();
    if target.kind == TransportKind::Tcp {
        engine.before_ip_connect().await;
    }
    let res: Result<Vec<u8>, TransportError> = match target.kind {
        TransportKind::Rtu | TransportKind::Ascii => match serial_transport.as_mut() {
            Some(t) => { engine.before_tx().await; t.transact(&adu, timeout, 32).await },
            None => return,
        },
        TransportKind::Tcp => match TcpTransport::connect(crate::transport::tcp::TcpConfig {
            host: target.endpoint.clone(),
            port: target.port.unwrap_or(502),
            connect_timeout: timeout,
            framing: engine.tcp_framing,
        tls: engine.tls.clone(),
        })
        .await
        {
            Ok(mut t) => {
                engine.before_tx().await;
                let out = t.transact(&adu, timeout, 32).await;
                let _ = t.close().await;
                out
            }
            Err(e) => Err(e),
        },
        TransportKind::Udp => match UdpTransport::connect(&target.endpoint, target.port.unwrap_or(502)).await {
            Ok(mut t) => { engine.before_tx().await; t.transact(&adu, timeout, 32).await },
            Err(e) => Err(e),
        },
    };
    let tx_status = match &res {
        Ok(_) => "ok",
        Err(TransportError::Timeout(_)) => "timeout",
        Err(_) => "error",
    };
    log_traffic(
        sink,
        target,
        Some(0x2B),
        &adu,
        res.as_ref().ok(),
        started.elapsed().as_micros(),
        tx_status,
    );

    let resp = match res {
        Ok(r) => r,
        Err(_) => return,
    };
    let pdu = match verify_adu(target.kind, target.unit_id, 0x2B, &resp, transaction_id, engine.clone()) {
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
        emit_event(sink, &ScanEvent::DeviceUpdated { record });
    }
}

/// Settings currently required by a serial work item (None for IP targets).
pub(crate) fn serial_signature(target: &DeviceTarget) -> Option<String> {
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
pub(crate) async fn ensure_serial_settings(
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

/// Append one raw exchange to the traffic log and stream it to the UI.
fn log_traffic(
    sink: &EventSink,
    target: &DeviceTarget,
    function: Option<u8>,
    tx: &[u8],
    rx: Option<&Vec<u8>>,
    rtt_us: u128,
    status: &str,
) {
    let record = TrafficRecord {
        id: 0,
        timestamp_ms: now_ms(),
        kind: target.kind,
        endpoint: target.endpoint.clone(),
        unit_id: target.unit_id,
        function,
        tx_hex: hex::encode(tx),
        rx_hex: rx.map(|v| hex::encode(v)),
        rtt_us,
        status: status.to_string(),
    };
    sink.traffic(record.clone());
    if let Ok(value) = serde_json::to_value(&record) {
        sink.emit("traffic:record", value);
    }
}

/// Emit one `probe_completed` event per finished work item.
fn emit_probe_completed(sink: &EventSink, key: &str, function: u8, result: &ProbeResult) {
    emit_event(
        sink,
        &ScanEvent::ProbeCompleted {
            device_key: key.to_string(),
            function,
            result: result.clone(),
        },
    );
}

fn emit_event(sink: &EventSink, event: &ScanEvent) {
    if let Ok(value) = serde_json::to_value(event) {
        sink.emit("scan:event", value);
    }
}

fn emit_progress(
    sink: &EventSink,
    status: ScanStatus,
    total: u64,
    done: u64,
    results: &Arc<RwLock<ScanResults>>,
) {
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
    if let Ok(value) = serde_json::to_value(&progress) {
        sink.emit("scan:progress", value);
    }
}

//! Scanner types: the IPC contract between the Rust backend and the Vue frontend.
//!
//! Everything here is `Serialize + Deserialize` + `Clone` and is mirrored on the
//! frontend in `src/types/ipc.ts`.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::modbus::common::TransportKind;
use crate::transport::serial::{DataBitsSerde, ParitySerde, StopBitsSerde};

pub use crate::modbus::common::SerialMode;

/// What to scan: the target space.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScanTarget {
    /// Serial: scan a range of unit (slave) ids on one port.
    UnitIdRange {
        from: u8,
        to: u8,
        /// Serial port settings.
        port: SerialPortParams,
        /// RTU or ASCII.
        mode: SerialMode,
    },
    /// TCP/UDP: scan a range of IP addresses (one port each).
    IpRange {
        /// "192.168.1.1" - "192.168.1.255" or a CIDR "192.168.1.0/24"
        from: String,
        to: Option<String>,
        /// Destination TCP/UDP port (502 by default).
        port: u16,
        kind: IpTransportKind,
    },
    /// Single device, deep register map scan.
    SingleDevice {
        target: DeviceTarget,
        /// Register ranges to probe per function code.
        register_plan: Vec<RegisterRange>,
    },
}

/// Serial port parameters (IPC form).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialPortParams {
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: DataBitsSerde,
    pub parity: ParitySerde,
    pub stop_bits: StopBitsSerde,
}

/// IP transport kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IpTransportKind {
    Tcp,
    Udp,
}

/// One device address (serial unit id or IP + unit id for gateways).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceTarget {
    pub kind: TransportKind,
    /// Serial: port name; TCP/UDP: host.
    pub endpoint: String,
    /// TCP/UDP port.
    pub port: Option<u16>,
    /// Unit id (TCP: usually 1 or 255; gateways route to serial slaves).
    pub unit_id: u8,
    pub serial: Option<SerialPortParams>,
    pub serial_mode: Option<SerialMode>,
}

/// A register/coil range to probe with a function code.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RegisterRange {
    /// Function code (0x01..=0x04 typically).
    pub function: u8,
    /// Start address.
    pub start: u16,
    /// Quantity (1..=125 for registers).
    pub quantity: u16,
}

/// How to scan (timing and behaviour).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanParams {
    /// Response timeout per request.
    pub timeout_ms: u32,
    /// Delay between consecutive requests (rate limiting).
    pub request_gap_ms: u32,
    /// Retries for a failed/timed-out request before marking as "no response".
    pub retries: u32,
    /// Probe function codes for device discovery.
    pub probe_functions: Vec<u8>,
    /// Stop the whole scan after the first found device (unit/IP).
    pub stop_on_first: bool,
    /// Register plan applied to every discovered device.
    pub register_plan: Vec<RegisterRange>,
    /// Concurrency for IP scans (max in-flight sockets).
    pub ip_concurrency: u32,
}

impl Default for ScanParams {
    fn default() -> Self {
        Self {
            timeout_ms: 800,
            request_gap_ms: 20,
            retries: 1,
            probe_functions: vec![0x03, 0x04, 0x01, 0x02],
            stop_on_first: false,
            register_plan: Vec::new(),
            ip_concurrency: 32,
        }
    }
}

/// Result of a single probe (one request to one target).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProbeResult {
    /// Device answered with data.
    Ok {
        function: u8,
        /// Parsed register values (u16) or coil bits, depending on the function.
        values: Vec<u16>,
        /// Raw response PDU (hex) for the raw view.
        raw_hex: String,
        /// Round-trip time in microseconds.
        rtt_us: u128,
    },
    /// Device answered with a Modbus exception.
    Exception {
        function: u8,
        code: u8,
        /// Human-readable error (Russian/English by i18n on the frontend).
        message: String,
        rtt_us: u128,
    },
    /// No response (timeout x retries).
    NoResponse {
        function: u8,
    },
    /// Transport-level error (port closed, unreachable, etc).
    TransportError {
        function: u8,
        message: String,
    },
}

/// One discovered device + all its collected data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecord {
    /// Unique target key, e.g. "rtu:COM3:1", "tcp:192.168.1.5:502:1".
    pub key: String,
    pub target: DeviceTarget,
    /// Results per probed function code (map: function -> result).
    pub probes: HashMap<u8, ProbeResult>,
    /// Full register map scan results (key: "fc:address").
    pub registers: HashMap<String, RegisterValue>,
    /// First seen timestamp (unix ms).
    pub first_seen_ms: u64,
    /// Last activity timestamp (unix ms).
    pub last_seen_ms: u64,
}

/// A single scanned register value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterValue {
    pub function: u8,
    pub address: u16,
    /// Raw u16 value (or 0/1 for coils).
    pub raw: u16,
    /// Decoded views for the UI.
    pub as_hex: String,
    /// RTT of the read.
    pub rtt_us: u128,
}

/// Scan status for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    /// Not started.
    Idle,
    /// Preparing (opening ports etc).
    Preparing,
    /// Running.
    Running,
    /// Paused by user.
    Paused,
    /// Stopped by user.
    Stopped,
    /// Finished normally.
    Finished,
    /// Failed to start (port busy, bad params).
    Failed,
}

/// Live progress for the UI (emitted as event `scan:progress`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub status: ScanStatus,
    /// Total work items (targets x probes).
    pub total: u64,
    /// Completed work items.
    pub done: u64,
    /// Devices found so far.
    pub found: u64,
    /// Current target being probed (display string).
    pub current: String,
    /// Errors so far (timeouts, io).
    pub errors: u64,
    /// Elapsed time in ms.
    pub elapsed_ms: u64,
}

/// Event emitted to the frontend: `scan:event` (device found, status change...).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScanEvent {
    StatusChanged {
        status: ScanStatus,
        message: String,
    },
    DeviceFound {
        record: DeviceRecord,
    },
    DeviceUpdated {
        record: DeviceRecord,
    },
    ProbeCompleted {
        device_key: String,
        function: u8,
        result: ProbeResult,
    },
    Finished {
        total: u64,
        found: u64,
        errors: u64,
        elapsed_ms: u64,
    },
}

/// Current time in unix milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Result of the whole scan session (returned by `get_scan_results`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanResults {
    pub records: Vec<DeviceRecord>,
    pub stats: ScanStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanStats {
    pub requests_sent: u64,
    pub responses_ok: u64,
    pub responses_exception: u64,
    pub timeouts: u64,
    pub transport_errors: u64,
    pub started_ms: Option<u64>,
    pub finished_ms: Option<u64>,
}

impl ScanStats {
    pub fn elapsed_ms(&self) -> u64 {
        match (self.started_ms, self.finished_ms) {
            (Some(s), Some(f)) => f.saturating_sub(s),
            (Some(s), None) => now_ms().saturating_sub(s),
            _ => 0,
        }
    }
}

//! Scanner types: the IPC contract between the Rust backend and the Vue frontend.
//!
//! Everything here is `Serialize + Deserialize` + `Clone` and is mirrored on the
//! frontend in `src/types/ipc.ts`.

use ts_rs::TS;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::modbus::common::TransportKind;
use crate::modbus::decode::DecodeSpec;
use crate::modbus::frame::DeviceIdentification;
use crate::transport::tcp::TcpFraming;
use crate::transport::tls::TlsParams;
use crate::transport::serial::{DataBitsSerde, ParitySerde, StopBitsSerde};

pub use crate::modbus::common::SerialMode;

/// What to scan: the target space.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
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
    /// Serial: unknown baud/framing — sweep settings, then scan unit IDs.
    SerialAuto {
        sweep: SerialSweepParams,
    },
    /// Adaptive register/coil map scan on one device (P2-03).
    RegisterScan {
        scan: RegisterScanParams,
    },
    /// Single device, deep register map scan.
    SingleDevice {
        target: DeviceTarget,
        /// Register ranges to probe per function code.
        register_plan: Vec<RegisterRange>,
    },
}

/// Framing (data bits / parity / stop bits) for a serial sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SerialFraming {
    pub data_bits: DataBitsSerde,
    pub parity: ParitySerde,
    pub stop_bits: StopBitsSerde,
}

/// Parameters for a two-phase serial settings sweep (P2-01).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SerialSweepParams {
    pub port_name: String,
    pub mode: SerialMode,
    /// Baud rates in priority order (e.g. 9600, 2400, 19200 ...).
    pub baud_rates: Vec<u32>,
    /// Framings in priority order (e.g. 8N1, 8E1, 8O1).
    pub framings: Vec<SerialFraming>,
    /// Few unit ids probed while looking for the correct settings.
    pub discovery_unit_ids: Vec<u8>,
    /// Unit-id range scanned after the settings are locked.
    pub unit_from: u8,
    pub unit_to: u8,
}

impl SerialSweepParams {
    /// Field-practice defaults: 8N1 → 8E1 → 8O1 over common bauds.
    pub fn defaults(port_name: String, mode: SerialMode, unit_from: u8, unit_to: u8) -> Self {
        use crate::transport::serial::{DataBitsSerde, ParitySerde, StopBitsSerde};
        let framing = |parity| SerialFraming {
            data_bits: DataBitsSerde::Eight,
            parity,
            stop_bits: StopBitsSerde::One,
        };
        Self {
            port_name,
            mode,
            baud_rates: vec![9600, 2400, 19200, 4800, 38400, 57600, 115200, 1200],
            framings: vec![
                framing(ParitySerde::None),
                framing(ParitySerde::Even),
                framing(ParitySerde::Odd),
            ],
            discovery_unit_ids: vec![1, 2, 3, 4, 5, 10, 20, 100, 247],
            unit_from,
            unit_to,
        }
    }
}

/// Serial port parameters (IPC form).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SerialPortParams {
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: DataBitsSerde,
    pub parity: ParitySerde,
    pub stop_bits: StopBitsSerde,
}

/// Adaptive register-scan strategy (field-practice strategies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum RegisterScanMode {
    /// Blocks of `batch`; a failed block is split into fifths once, then refined to singles.
    Quick,
    /// 100 → 20 → 5, then edge refinement register by register (default).
    Balanced,
    /// Split failed blocks all the way down to single registers.
    Deep,
}

impl Default for RegisterScanMode {
    fn default() -> Self {
        Self::Balanced
    }
}

/// Parameters for an adaptive register-map scan.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RegisterScanParams {
    pub target: DeviceTarget,
    /// Function code: 0x01..=0x04.
    pub function: u8,
    /// First address (inclusive).
    pub start: u16,
    /// Last address (inclusive).
    pub end: u16,
    pub mode: RegisterScanMode,
    /// Initial block size (0 = strategy default).
    #[serde(default)]
    pub batch: u16,
}

/// IP transport kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum IpTransportKind {
    Tcp,
    Udp,
}

/// One device address (serial unit id or IP + unit id for gateways).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
pub struct RegisterRange {
    /// Function code (0x01..=0x04 typically).
    pub function: u8,
    /// Start address.
    pub start: u16,
    /// Quantity (1..=125 for registers).
    pub quantity: u16,
}

/// How to scan (timing and behaviour).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
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
    /// Try FC43/MEI14 identification after a device answers (P2-05).
    #[serde(default = "default_identify_devices")]
    pub identify_devices: bool,
    /// TCP wire format: standard MBAP or raw RTU over TCP.
    #[serde(default)]
    pub tcp_framing: TcpFraming,
    /// Use MBAP framing for UDP instead of the default RTU-style ADU.
    #[serde(default)]
    pub udp_mbap: bool,
    /// Modbus Security: wrap TCP connections in TLS (port 802 by convention).
    #[serde(default)]
    pub tls: Option<TlsParams>,
}

fn default_identify_devices() -> bool {
    true
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
            identify_devices: true,
            tcp_framing: TcpFraming::Mbap,
            udp_mbap: false,
            tls: None,
        }
    }
}

/// Result of a single probe (one request to one target).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
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
        #[ts(type = "number")]
        rtt_us: u128,
    },
    /// Device answered with a Modbus exception.
    Exception {
        function: u8,
        code: u8,
        /// Human-readable error (Russian/English by i18n on the frontend).
        message: String,
        #[ts(type = "number")]
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

/// One register description inside a reusable device profile (E-04).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ProfileRegister {
    pub function: u8,
    pub address: u16,
    pub name: String,
    pub spec: DecodeSpec,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// A reusable register map matched by FC43 identity.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DeviceProfile {
    pub id: String,
    pub vendor: String,
    pub product_code: String,
    pub revision: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub registers: Vec<ProfileRegister>,
}

impl DeviceProfile {
    /// Identity fields are case-insensitive; an empty profile field is a wildcard.
    pub fn matches(&self, identity: &DeviceIdentification) -> bool {
        let vendor = identity.vendor_name().unwrap_or("").trim().to_lowercase();
        let product = identity.product_code().unwrap_or("").trim().to_lowercase();
        let revision = identity.revision().unwrap_or("").trim().to_lowercase();
        let matches_field = |expected: &str, actual: &str| {
            expected.trim().is_empty() || expected.trim().to_lowercase() == actual
        };
        matches_field(&self.vendor, &vendor)
            && matches_field(&self.product_code, &product)
            && matches_field(&self.revision, &revision)
    }

    /// Build a skeleton profile from a discovered device's identity.
    pub fn from_identity(identity: &DeviceIdentification) -> Self {
        let vendor = identity.vendor_name().unwrap_or("unknown").to_string();
        let product = identity.product_code().unwrap_or("unknown").to_string();
        let revision = identity.revision().unwrap_or("").to_string();
        let id = format!("{vendor}|{product}|{revision}").to_lowercase().replace(' ', "_");
        let name = format!("{vendor} {product}");
        Self {
            id,
            vendor,
            product_code: product,
            revision,
            name,
            description: None,
            registers: Vec::new(),
        }
    }

    /// Turn the profile's register descriptions into tags for one device.
    pub fn to_tags(&self, device_key: &str) -> Vec<TagDefinition> {
        self.registers
            .iter()
            .map(|r| TagDefinition {
                device_key: device_key.to_string(),
                address: r.address,
                name: r.name.clone(),
                spec: r.spec,
                unit: r.unit.clone(),
                description: r.description.clone(),
            })
            .collect()
    }
}

/// A named register tag with decoding/scaling (P3-03).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TagDefinition {
    pub device_key: String,
    pub address: u16,
    pub name: String,
    pub spec: DecodeSpec,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// One discovered device + all its collected data.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DeviceRecord {
    /// Unique target key, e.g. "rtu:COM3:1", "tcp:192.168.1.5:502:1".
    pub key: String,
    pub target: DeviceTarget,
    /// Results per probed function code (map: function -> result).
    pub probes: HashMap<u8, ProbeResult>,
    /// Full register map scan results (key: "fc:address").
    pub registers: HashMap<String, RegisterValue>,
    /// FC43/MEI14 device identity, if supported (P2-05).
    #[serde(default)]
    pub device_identification: Option<DeviceIdentification>,
    /// First seen timestamp (unix ms).
    #[ts(type = "number")]
    pub first_seen_ms: u64,
    /// Last activity timestamp (unix ms).
    #[ts(type = "number")]
    pub last_seen_ms: u64,
}

/// A single scanned register value.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RegisterValue {
    pub function: u8,
    pub address: u16,
    /// Raw u16 value (or 0/1 for coils).
    pub raw: u16,
    /// Decoded views for the UI.
    pub as_hex: String,
    /// RTT of the read.
    #[ts(type = "number")]
    pub rtt_us: u128,
}

/// Scan status for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
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
    /// Cancellation requested; existing I/O is being torn down.
    Stopping,
    /// Stopped by user.
    Stopped,
    /// Finished normally.
    Finished,
    /// Failed to start (port busy, bad params).
    Failed,
}

/// Live progress for the UI (emitted as event `scan:progress`).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ScanProgress {
    pub status: ScanStatus,
    /// Total work items (targets x probes).
    #[ts(type = "number")]
    pub total: u64,
    /// Completed work items.
    #[ts(type = "number")]
    pub done: u64,
    /// Devices found so far.
    #[ts(type = "number")]
    pub found: u64,
    /// Current target being probed (display string).
    pub current: String,
    /// Errors so far (timeouts, io).
    #[ts(type = "number")]
    pub errors: u64,
    /// Elapsed time in ms.
    #[ts(type = "number")]
    pub elapsed_ms: u64,
}

/// Event emitted to the frontend: `scan:event` (device found, status change...).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
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
    /// IP target was classified during discovery (P2-04).
    IpTargetClassified {
        status: IpTargetStatus,
    },
    Finished {
        #[ts(type = "number")]
        total: u64,
        #[ts(type = "number")]
        found: u64,
        #[ts(type = "number")]
        errors: u64,
        #[ts(type = "number")]
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

/// What we learned about one scanned IP endpoint (P2-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum IpTargetClass {
    /// At least one valid Modbus answer (data or exception).
    ValidModbus,
    /// TCP connection succeeded, but nothing spoke Modbus.
    PortOpenNoModbus,
    /// TCP connection was actively refused.
    ConnectionRefused,
    /// No answer within the configured timeout.
    Timeout,
    /// Host/network unreachable.
    Unreachable,
    /// Not probed yet.
    Unknown,
}

impl IpTargetClass {
    /// Higher rank wins when several unit ids are probed behind one IP.
    pub fn rank(self) -> u8 {
        match self {
            IpTargetClass::ValidModbus => 5,
            IpTargetClass::PortOpenNoModbus => 4,
            IpTargetClass::ConnectionRefused => 3,
            IpTargetClass::Timeout => 2,
            IpTargetClass::Unreachable => 1,
            IpTargetClass::Unknown => 0,
        }
    }
}

/// Aggregated classification for one `ip:port` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IpTargetStatus {
    pub endpoint: String,
    pub port: u16,
    pub class: IpTargetClass,
    pub unit_ids_probed: Vec<u8>,
    pub responses_ok: u32,
    pub responses_exception: u32,
    pub timeouts: u32,
    pub transport_errors: u32,
    #[ts(type = "number")]
    pub last_seen_ms: u64,
}

/// Kind of change for one register between two scans (P3-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
    Unchanged,
}

/// One register-level difference.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RegisterDiff {
    pub device_key: String,
    pub function: u8,
    pub address: u16,
    pub change: ChangeKind,
    pub old_value: Option<u16>,
    pub new_value: Option<u16>,
}

/// Per-device diff counters.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DeviceDiff {
    pub device_key: String,
    pub added: u32,
    pub removed: u32,
    pub changed: u32,
}

/// Result of comparing two scan snapshots.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ScanDiff {
    pub left_label: String,
    pub right_label: String,
    pub left_devices: usize,
    pub right_devices: usize,
    pub added_devices: Vec<String>,
    pub removed_devices: Vec<String>,
    pub devices: Vec<DeviceDiff>,
    pub registers: Vec<RegisterDiff>,
    pub added: u32,
    pub removed: u32,
    pub changed: u32,
}

/// Output format of a file logger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Csv,
    Json,
    Tsv,
}

/// File rotation policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RotationMode {
    /// Single file, no rotation.
    None,
    /// New file every day (date suffix).
    Daily,
    /// New file per monitor session (timestamp suffix).
    Session,
    /// Rotate when the file exceeds N megabytes.
    SizeMb(u32),
}

/// One file logger attached to monitor samples (P3-07).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LoggerConfig {
    pub id: String,
    pub path: String,
    pub format: LogFormat,
    pub rotation: RotationMode,
    /// Only these watch keys are written (empty = all).
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default = "default_logger_enabled")]
    pub enabled: bool,
}

fn default_logger_enabled() -> bool {
    true
}

/// Runtime status of a file logger.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LoggerStatus {
    pub id: String,
    pub path: String,
    pub format: LogFormat,
    pub rotation: RotationMode,
    pub enabled: bool,
    #[ts(type = "number")]
    pub records_written: u64,
    pub current_file: String,
    #[ts(type = "number | null")]
    pub last_write_ms: Option<u64>,
    pub error: Option<String>,
}

/// One raw Modbus exchange for protocol debugging (P3-06).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TrafficRecord {
    #[ts(type = "number")]
    pub id: u64,
    #[ts(type = "number")]
    pub timestamp_ms: u64,
    pub kind: TransportKind,
    pub endpoint: String,
    pub unit_id: u8,
    /// Function code when known (0x2B for identification).
    pub function: Option<u8>,
    pub tx_hex: String,
    /// Raw response ADU, if any.
    pub rx_hex: Option<String>,
    #[ts(type = "number")]
    pub rtt_us: u128,
    /// `ok`, `timeout`, `error`, `invalid_response`...
    pub status: String,
}

/// One register address watched by the live monitor (P3-05).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MonitorWatch {
    pub key: String,
    pub target: DeviceTarget,
    pub function: u8,
    pub address: u16,
    pub quantity: u16,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
}

/// One poll result for a watch.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MonitorSample {
    #[ts(type = "number")]
    pub timestamp_ms: u64,
    pub values: Vec<u16>,
    #[ts(type = "number")]
    pub rtt_us: u128,
}

/// Aggregated state of a watched register.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MonitorValueState {
    pub key: String,
    pub name: String,
    pub unit: String,
    pub values: Vec<u16>,
    pub min: Option<u16>,
    pub max: Option<u16>,
    /// True when the last poll differed from the previous one.
    pub changed: bool,
    pub samples: Vec<MonitorSample>,
    pub errors: u32,
}

/// Event emitted once per monitor poll cycle.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MonitorUpdate {
    pub values: Vec<MonitorValueState>,
    #[ts(type = "number")]
    pub cycle_ms: u64,
}

/// One saved scan snapshot inside a project (P3-04).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ScanSnapshot {
    #[ts(type = "number")]
    pub timestamp_ms: u64,
    pub label: String,
    pub results: ScanResults,
}

/// A portable project: devices, tag map and scan history in one file.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Project {
    pub name: String,
    pub version: u32,
    #[ts(type = "number")]
    pub created_ms: u64,
    #[ts(type = "number")]
    pub updated_ms: u64,
    pub results: ScanResults,
    pub tags: Vec<TagDefinition>,
    pub history: Vec<ScanSnapshot>,
}

/// Result of the whole scan session (returned by `get_scan_results`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
pub struct ScanResults {
    pub records: Vec<DeviceRecord>,
    #[serde(default)]
    pub ip_targets: Vec<IpTargetStatus>,
    pub stats: ScanStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
pub struct ScanStats {
    #[ts(type = "number")]
    pub requests_sent: u64,
    #[ts(type = "number")]
    pub responses_ok: u64,
    #[ts(type = "number")]
    pub responses_exception: u64,
    #[ts(type = "number")]
    pub timeouts: u64,
    #[ts(type = "number")]
    pub transport_errors: u64,
    #[ts(type = "number | null")]
    pub started_ms: Option<u64>,
    #[ts(type = "number | null")]
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

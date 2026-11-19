/**
 * IPC contract types — TypeScript mirror of `src-tauri/src/scanner/types.rs`,
 * `src-tauri/src/commands/*` and `src-tauri/src/transport/serial.rs`.
 *
 * IMPORTANT: any change on the Rust side must be mirrored here (and vice versa).
 */

// ---------------------------------------------------------------------------
// Common / transport
// ---------------------------------------------------------------------------

export type TransportKind = "rtu" | "ascii" | "tcp" | "udp";

export type DataBitsSerde = "five" | "six" | "seven" | "eight";
export type ParitySerde = "none" | "odd" | "even";
export type StopBitsSerde = "one" | "two";

export interface SerialPortParams {
  port_name: string;
  baud_rate: number;
  data_bits: DataBitsSerde;
  parity: ParitySerde;
  stop_bits: StopBitsSerde;
}

export type SerialMode = "rtu" | "ascii";
export type IpTransportKind = "tcp" | "udp";

// ---------------------------------------------------------------------------
// Scan targets
// ---------------------------------------------------------------------------

export interface UnitIdRange {
  type: "unit_id_range";
  from: number;
  to: number;
  port: SerialPortParams;
  mode: SerialMode;
}

/** One baud/framing combination for an automatic serial sweep. */
export interface SerialFraming {
  data_bits: DataBitsSerde;
  parity: ParitySerde;
  stop_bits: StopBitsSerde;
}

/** P2-01: two-phase sweep of serial settings, then unit-id scan. */
export interface SerialSweepParams {
  port_name: string;
  mode: SerialMode;
  /** Priority order, e.g. [9600, 2400, 19200, ...]. */
  baud_rates: number[];
  /** Priority order, e.g. 8N1, 8E1, 8O1. */
  framings: SerialFraming[];
  /** Few ids probed while looking for the right settings. */
  discovery_unit_ids: number[];
  /** Full range scanned after the settings are locked. */
  unit_from: number;
  unit_to: number;
}

export interface SerialAuto {
  type: "serial_auto";
  sweep: SerialSweepParams;
}

export interface TlsParams {
  client_cert?: string | null;
  client_key?: string | null;
}

export type RegisterScanMode = "quick" | "balanced" | "deep";

/** P2-03: adaptive register/coil map scan on a single device. */
export interface RegisterScanParams {
  target: ManualTarget;
  function: number;
  start: number;
  end: number;
  mode: RegisterScanMode;
  /** 0 = mode default (100 for Quick/Balanced, 20 for Deep). */
  batch: number;
}

export interface RegisterScanTarget {
  type: "register_scan";
  scan: RegisterScanParams;
}

export interface IpRange {
  type: "ip_range";
  /** "192.168.1.1" or CIDR "192.168.1.0/24" */
  from: string;
  to?: string | null;
  port: number;
  kind: IpTransportKind;
}

export interface SingleDevice {
  type: "single_device";
  target: DeviceTarget;
  register_plan: RegisterRange[];
}

export type ScanTarget =
  | UnitIdRange
  | SerialAuto
  | IpRange
  | SingleDevice
  | RegisterScanTarget;

export interface DeviceTarget {
  kind: TransportKind;
  endpoint: string;
  port?: number | null;
  unit_id: number;
  serial?: SerialPortParams | null;
  serial_mode?: SerialMode | null;
}

export interface RegisterRange {
  function: number;
  start: number;
  quantity: number;
}

// ---------------------------------------------------------------------------
// Scan params / results
// ---------------------------------------------------------------------------

export interface ScanParams {
  timeout_ms: number;
  request_gap_ms: number;
  retries: number;
  probe_functions: number[];
  stop_on_first: boolean;
  register_plan: RegisterRange[];
  ip_concurrency: number;
  /** Try FC43/MEI14 identification after a device answers. */
  identify_devices?: boolean;
  /** TCP wire format: standard MBAP or raw RTU over TCP. */
  tcp_framing?: "mbap" | "rtu";
  /** MBAP framing for UDP instead of the default RTU-style ADU. */
  udp_mbap?: boolean;
  /** Modbus Security (TLS, port 802); null = plain TCP. */
  tls?: TlsParams | null;
}

export type ProbeResult =
  | { status: "ok"; function: number; values: number[]; raw_hex: string; rtt_us: number }
  | { status: "exception"; function: number; code: number; message: string; rtt_us: number }
  | { status: "no_response"; function: number }
  | { status: "transport_error"; function: number; message: string };

export interface RegisterValue {
  function: number;
  address: number;
  raw: number;
  as_hex: string;
  rtt_us: number;
}

export type ByteOrder = "abcd" | "badc" | "cdab" | "dcba";
export type DataType = "int16" | "uint16" | "int32" | "uint32" | "float32" | "ascii";

export interface DecodeSpec {
  data_type: DataType;
  byte_order: ByteOrder;
  multiplier?: number;
  divider?: number;
  offset?: number;
}

export type DecodedValue =
  | { kind: "integer"; value: number }
  | { kind: "float"; value: number }
  | { kind: "text"; value: string };

export interface Interpretation {
  data_type: DataType;
  byte_order: ByteOrder;
  value: DecodedValue;
}

export interface DecodeSuggestion {
  data_type: DataType;
  byte_order: ByteOrder;
  value: number;
  reason: string;
}

export interface DecodeResult {
  interpretations: Interpretation[];
  suggestions: DecodeSuggestion[];
}

export interface DeviceIdObject {
  id: number;
  value: string;
}

export interface DeviceIdentification {
  read_code: number;
  conformity_level: number;
  more_follows: boolean;
  next_object_id: number;
  objects: DeviceIdObject[];
}

export interface ProfileRegister {
  function: number;
  address: number;
  name: string;
  spec: DecodeSpec;
  unit?: string | null;
  description?: string | null;
}

export interface DeviceProfile {
  id: string;
  vendor: string;
  product_code: string;
  revision: string;
  name: string;
  description?: string | null;
  registers: ProfileRegister[];
}

export interface TagDefinition {
  device_key: string;
  address: number;
  name: string;
  spec: DecodeSpec;
  unit?: string;
  description?: string;
}

export interface DeviceRecord {
  key: string;
  target: DeviceTarget;
  /** map: function code -> result */
  probes: Record<number, ProbeResult>;
  /** map: "fc:address" -> value */
  registers: Record<string, RegisterValue>;
  device_identification?: DeviceIdentification | null;
  first_seen_ms: number;
  last_seen_ms: number;
}

export type ScanStatus =
  | "idle"
  | "preparing"
  | "running"
  | "paused"
  | "stopped"
  | "finished"
  | "failed";

export interface ScanProgress {
  status: ScanStatus;
  total: number;
  done: number;
  found: number;
  current: string;
  errors: number;
  elapsed_ms: number;
}

export type IpTargetClass =
  | "valid_modbus"
  | "port_open_no_modbus"
  | "connection_refused"
  | "timeout"
  | "unreachable"
  | "unknown";

export interface IpTargetStatus {
  endpoint: string;
  port: number;
  class: IpTargetClass;
  unit_ids_probed: number[];
  responses_ok: number;
  responses_exception: number;
  timeouts: number;
  transport_errors: number;
  last_seen_ms: number;
}

export type ScanEvent =
  | { kind: "status_changed"; status: ScanStatus; message: string }
  | { kind: "device_found"; record: DeviceRecord }
  | { kind: "device_updated"; record: DeviceRecord }
  | { kind: "ip_target_classified"; status: IpTargetStatus }
  | { kind: "probe_completed"; device_key: string; function: number; result: ProbeResult }
  | { kind: "finished"; total: number; found: number; errors: number; elapsed_ms: number };

export type LogFormat = "csv" | "json" | "tsv";
export type RotationMode =
  | "none"
  | "daily"
  | "session"
  | { size_mb: number };

export interface LoggerConfig {
  id: string;
  path: string;
  format: LogFormat;
  rotation: RotationMode;
  keys?: string[];
  enabled?: boolean;
}

export interface LoggerStatus {
  id: string;
  path: string;
  format: LogFormat;
  rotation: RotationMode;
  enabled: boolean;
  records_written: number;
  current_file: string;
  last_write_ms?: number | null;
  error?: string | null;
}

export interface TrafficRecord {
  id: number;
  timestamp_ms: number;
  kind: TransportKind;
  endpoint: string;
  unit_id: number;
  function?: number | null;
  tx_hex: string;
  rx_hex?: string | null;
  rtt_us: number;
  status: string;
}

export interface MonitorWatch {
  key: string;
  target: ManualTarget;
  function: number;
  address: number;
  quantity: number;
  name?: string | null;
  unit?: string | null;
}

export interface MonitorSample {
  timestamp_ms: number;
  values: number[];
  rtt_us: number;
}

export interface MonitorValueState {
  key: string;
  name: string;
  unit: string;
  values: number[];
  min?: number | null;
  max?: number | null;
  changed: boolean;
  samples: MonitorSample[];
  errors: number;
}

export interface MonitorUpdate {
  values: MonitorValueState[];
  cycle_ms: number;
}

export type ChangeKind = "added" | "removed" | "changed" | "unchanged";

export interface RegisterDiff {
  device_key: string;
  function: number;
  address: number;
  change: ChangeKind;
  old_value?: number | null;
  new_value?: number | null;
}

export interface DeviceDiff {
  device_key: string;
  added: number;
  removed: number;
  changed: number;
}

export interface ScanDiff {
  left_label: string;
  right_label: string;
  left_devices: number;
  right_devices: number;
  added_devices: string[];
  removed_devices: string[];
  devices: DeviceDiff[];
  registers: RegisterDiff[];
  added: number;
  removed: number;
  changed: number;
}

export interface ScanSnapshot {
  timestamp_ms: number;
  label: string;
  results: ScanResults;
}

export interface Project {
  name: string;
  version: number;
  created_ms: number;
  updated_ms: number;
  results: ScanResults;
  tags: TagDefinition[];
  history: ScanSnapshot[];
}

export interface ScanStats {
  requests_sent: number;
  responses_ok: number;
  responses_exception: number;
  timeouts: number;
  transport_errors: number;
  started_ms?: number | null;
  finished_ms?: number | null;
}

export interface ScanResults {
  records: DeviceRecord[];
  ip_targets: IpTargetStatus[];
  stats: ScanStats;
}

// ---------------------------------------------------------------------------
// Serial ports (system)
// ---------------------------------------------------------------------------

export interface SerialPortInfo {
  port_name: string;
  port_type: string;
  vid?: number | null;
  pid?: number | null;
  manufacturer?: string | null;
  product?: string | null;
}

export interface TestSerialResult {
  opened: boolean;
  error?: string | null;
  modbus_responded: boolean;
  rtt_us?: number | null;
  response_hex?: string | null;
}

// ---------------------------------------------------------------------------
// Manual (debug) commands
// ---------------------------------------------------------------------------

export interface ManualTarget {
  kind: TransportKind;
  endpoint: string;
  port?: number | null;
  unit_id: number;
  serial?: SerialPortParams | null;
  serial_mode?: SerialMode | null;
}

export interface ManualTiming {
  timeout_ms: number;
  retries?: number;
}

export interface ReadRequest {
  target: ManualTarget;
  function: number;
  start_address: number;
  quantity: number;
  timing?: ManualTiming | null;
}

export interface WriteRequest {
  target: ManualTarget;
  function: number;
  start_address: number;
  values: number[];
  timing?: ManualTiming | null;
}

export interface RawFrameRequest {
  target: ManualTarget;
  adu_hex: string;
  is_full_adu: boolean;
  timing?: ManualTiming | null;
}

export interface ManualResponse {
  ok: boolean;
  error?: string | null;
  exception_code?: number | null;
  values: number[];
  response_hex?: string | null;
  rtt_us?: number | null;
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export interface Config {
  language: string;
  theme: string;
  scan_params: ScanParams;
  last_serial?: SerialPortParamsFull | null;
  last_tcp_host?: string | null;
  last_tcp_port?: number | null;
  last_udp_host?: string | null;
  last_udp_port?: number | null;
  export_dir?: string | null;
  export_format: string;
  tags?: TagDefinition[];
  profiles?: DeviceProfile[];
}

/** Backend Config.last_serial uses transport::serial::SerialPortConfig. */
export interface SerialPortParamsFull extends SerialPortParams {
  frame_gap_ms?: number;
}

export type ExportFormat = "csv" | "json" | "xml";

// ---------------------------------------------------------------------------
// Modbus constants (UI)
// ---------------------------------------------------------------------------

export const FC = {
  READ_COILS: 0x01,
  READ_DISCRETE_INPUTS: 0x02,
  READ_HOLDING_REGISTERS: 0x03,
  READ_INPUT_REGISTERS: 0x04,
  WRITE_SINGLE_COIL: 0x05,
  WRITE_SINGLE_REGISTER: 0x06,
  WRITE_MULTIPLE_COILS: 0x0f,
  WRITE_MULTIPLE_REGISTERS: 0x10,
  MASK_WRITE_REGISTER: 0x16,
  READ_WRITE_MULTIPLE_REGISTERS: 0x17,
} as const;

export const FC_NAMES: Record<number, string> = {
  0x01: "Read Coils (0x01)",
  0x02: "Read Discrete Inputs (0x02)",
  0x03: "Read Holding Registers (0x03)",
  0x04: "Read Input Registers (0x04)",
  0x05: "Write Single Coil (0x05)",
  0x06: "Write Single Register (0x06)",
  0x0f: "Write Multiple Coils (0x0F)",
  0x10: "Write Multiple Registers (0x10)",
  0x16: "Mask Write Register (0x16)",
  0x17: "Read/Write Multiple Registers (0x17)",
};

export const BAUD_RATES = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];

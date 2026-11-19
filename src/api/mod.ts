/**
 * Typed wrappers over the Tauri `invoke` + event API.
 * All backend errors arrive as `string`; helpers normalize them.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  Config,
  DecodeResult,
  DeviceProfile,
  DecodeSpec,
  DecodedValue,
  ExportFormat,
  LoggerConfig,
  LoggerStatus,
  ManualResponse,
  MonitorUpdate,
  MonitorValueState,
  MonitorWatch,
  Project,
  ReadRequest,
  RawFrameRequest,
  ScanDiff,
  ScanEvent,
  ScanProgress,
  ScanResults,
  ScanStatus,
  ScanTarget,
  ScanParams,
  SerialPortInfo,
  TagDefinition,
  TestSerialResult,
  TrafficRecord,
  WriteRequest,
} from "@/types/ipc";

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

export interface AppInfo {
  name: string;
  version: string;
  rustc: string;
  debug: boolean;
}

export const api = {
  system: {
    getAppInfo: () => invoke<AppInfo>("get_app_info"),
    getLogLevel: () => invoke<string>("get_log_level"),
  },

  serial: {
    listPorts: () => invoke<SerialPortInfo[]>("list_serial_ports"),
    testPort: (params: unknown) => invoke<TestSerialResult>("test_serial_port", { params }),
  },

  scanner: {
    start: (target: ScanTarget, params: ScanParams) =>
      invoke<void>("start_scan", { target, params }),
    stop: () => invoke<void>("stop_scan"),
    pause: () => invoke<void>("pause_scan"),
    resume: () => invoke<void>("resume_scan"),
    getStatus: () => invoke<ScanStatus>("get_scan_status"),
    getProgress: () => invoke<ScanProgress>("get_scan_progress"),
    getResults: () => invoke<ScanResults>("get_scan_results"),
    clearResults: () => invoke<void>("clear_results"),
    exportResults: (path: string, format: ExportFormat) =>
      invoke<number>("export_results", { path, format }),
    exportDevice: (key: string, path: string, format: ExportFormat) =>
      invoke<boolean>("export_device", { key, path, format }),
    saveProject: (path: string, name: string) =>
      invoke<string>("save_project", { path, name }),
    loadProject: (path: string) => invoke<Project>("load_project", { path }),
    startMonitor: (watches: MonitorWatch[], interval_ms: number) =>
      invoke<void>("start_monitor", { watches, interval_ms }),
    stopMonitor: () => invoke<void>("stop_monitor"),
    getMonitorState: () => invoke<MonitorValueState[]>("get_monitor_state"),
    getTrafficLog: () => invoke<TrafficRecord[]>("get_traffic_log"),
    clearTrafficLog: () => invoke<void>("clear_traffic_log"),
    exportTrafficLog: (path: string, format: ExportFormat) =>
      invoke<number>("export_traffic_log", { path, format }),
    setLoggers: (configs: LoggerConfig[]) =>
      invoke<LoggerStatus[]>("set_loggers", { configs }),
    getLoggers: () => invoke<LoggerStatus[]>("get_loggers"),
    compareScans: (
      left: ScanResults,
      right: ScanResults,
      left_label: string,
      right_label: string
    ) => invoke<ScanDiff>("compare_scans", { left, right, left_label, right_label }),
  },

  monitor: {
    onUpdate: (cb: (u: MonitorUpdate) => void) =>
      listen<MonitorUpdate>("monitor:update", (e) => cb(e.payload)),
  },

  profiles: {
    list: () => invoke<DeviceProfile[]>("get_profiles"),
    save: (profiles: DeviceProfile[]) =>
      invoke<DeviceProfile[]>("save_profiles", { profiles }),
    importFile: (path: string, format: "csv" | "json") =>
      invoke<DeviceProfile[]>("import_profiles", { path, format }),
    fromDevice: (deviceKey: string) =>
      invoke<DeviceProfile>("create_profile_from_device", { deviceKey }),
    suggest: (deviceKey: string) =>
      invoke<DeviceProfile | null>("suggest_profile", { deviceKey }),
    apply: (deviceKey: string, profileId: string) =>
      invoke<TagDefinition[]>("apply_profile", { deviceKey, profileId }),
  },

  modbus: {
    read: (req: ReadRequest) => invoke<ManualResponse>("read_registers", { req }),
    write: (req: WriteRequest) => invoke<ManualResponse>("write_registers", { req }),
    sendRaw: (req: RawFrameRequest) => invoke<ManualResponse>("send_raw_frame", { req }),
    inspectRegisters: (values: number[]) =>
      invoke<DecodeResult>("inspect_registers", { values }),
    decodeRegister: (values: number[], spec: DecodeSpec) =>
      invoke<DecodedValue | null>("decode_register", { values, spec }),
  },

  config: {
    get: () => invoke<Config>("get_config"),
    save: (config: Config) => invoke<void>("save_config", { config }),
    load: () => invoke<Config>("load_config"),
  },
};

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

export async function onScanProgress(cb: (p: ScanProgress) => void): Promise<UnlistenFn> {
  return listen<ScanProgress>("scan:progress", (e) => cb(e.payload));
}

export async function onScanEvent(cb: (e: ScanEvent) => void): Promise<UnlistenFn> {
  return listen<ScanEvent>("scan:event", (e) => cb(e.payload));
}

export async function onTrafficRecord(cb: (r: TrafficRecord) => void): Promise<UnlistenFn> {
  return listen<TrafficRecord>("traffic:record", (e) => cb(e.payload));
}

/**
 * Typed wrappers over the Tauri `invoke` + event API.
 * All backend errors arrive as `string`; helpers normalize them.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  Config,
  ExportFormat,
  ManualResponse,
  ReadRequest,
  RawFrameRequest,
  ScanEvent,
  ScanProgress,
  ScanResults,
  ScanStatus,
  ScanTarget,
  ScanParams,
  SerialPortInfo,
  TestSerialResult,
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
  },

  modbus: {
    read: (req: ReadRequest) => invoke<ManualResponse>("read_registers", { req }),
    write: (req: WriteRequest) => invoke<ManualResponse>("write_registers", { req }),
    sendRaw: (req: RawFrameRequest) => invoke<ManualResponse>("send_raw_frame", { req }),
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

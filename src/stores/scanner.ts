/**
 * Scanner store: scan lifecycle state + progress + event log.
 */

import { defineStore } from "pinia";
import { api, onScanEvent, onScanProgress } from "@/api/mod";
import type {
  ProbeResult,
  ScanEvent,
  ScanParams,
  ScanProgress,
  ScanStatus,
} from "@/types/ipc";

export interface LogEntry {
  time: Date;
  level: "info" | "warn" | "error" | "ok";
  message: string;
}

export const useScannerStore = defineStore("scanner", {
  state: () => ({
    status: "idle" as ScanStatus,
    progress: null as ScanProgress | null,
    log: [] as LogEntry[],
    listenersBound: false,
  }),

  getters: {
    isRunning: (s) => s.status === "running" || s.status === "preparing",
    isPaused: (s) => s.status === "paused",
    canStart: (s) => !["running", "preparing", "paused", "stopping"].includes(s.status),
  },

  actions: {
    addLog(level: LogEntry["level"], message: string) {
      this.log.push({ time: new Date(), level, message });
      if (this.log.length > 500) this.log.shift();
    },

    /** Subscribe to the backend events (call once from App.vue). */
    async bindEvents() {
      if (this.listenersBound) return;
      this.listenersBound = true;

      await onScanProgress((p) => {
        this.progress = p;
        this.status = p.status;
      });

      await onScanEvent((e) => this.handleEvent(e));
    },

    handleEvent(e: ScanEvent) {
      switch (e.kind) {
        case "status_changed":
          this.status = e.status;
          this.addLog("info", `status: ${e.status} (${e.message})`);
          break;
        case "device_found":
          this.addLog("ok", `device found: ${e.record.key}`);
          // the devices store refreshes from get_scan_results
          this.refreshDevicesSafe();
          break;
        case "device_updated":
        case "ip_target_classified":
          this.refreshDevicesSafe();
          break;
        case "probe_completed":
          this.logProbe(e.device_key, e.function, e.result);
          break;
        case "finished":
          // Terminal status comes from StatusChanged/scan:progress.
          // Do not overwrite an explicitly stopped scan with "finished".
          this.addLog(
            "info",
            `finished: done=${e.total} found=${e.found} errors=${e.errors} in ${e.elapsed_ms}ms`
          );
          break;
      }
    },

    /** One line per completed discovery probe (P2-09). */
    logProbe(deviceKey: string, functionCode: number, result: ProbeResult) {
      const fc = functionCode.toString(16).padStart(2, "0").toUpperCase();
      const level: LogEntry["level"] =
        result.status === "ok"
          ? "ok"
          : result.status === "exception"
            ? "warn"
            : result.status === "transport_error"
              ? "error"
              : "info";
      const rtt =
        "rtt_us" in result && result.rtt_us
          ? ` ${(result.rtt_us / 1000).toFixed(1)}ms`
          : "";
      let detail = "";
      if (result.status === "exception") {
        detail = ` exc ${result.code.toString(16).toUpperCase()}`;
      } else if (result.status === "transport_error") {
        detail = ` ${result.message}`;
      }
      this.addLog(level, `probe ${deviceKey} FC${fc} ${result.status}${detail}${rtt}`);
    },

    refreshDevicesSafe() {
      // Lazy import to avoid circular store dependencies.
      import("./devices").then(({ useDevicesStore }) => {
        useDevicesStore().refresh().catch(() => {});
      });
    },

    async start(target: Parameters<typeof api.scanner.start>[0], params: ScanParams) {
      await api.scanner.start(target, params);
      this.status = "preparing";
      this.addLog("info", "scan started");
    },

    async stop() {
      await api.scanner.stop();
      this.addLog("warn", "scan stop requested");
    },

    async pause() {
      await api.scanner.pause();
    },

    async resume() {
      await api.scanner.resume();
    },
  },
});

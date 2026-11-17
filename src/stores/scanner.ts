/**
 * Scanner store: scan lifecycle state + progress + event log.
 */

import { defineStore } from "pinia";
import { api, onScanEvent, onScanProgress } from "@/api/mod";
import type { ScanEvent, ScanParams, ScanProgress, ScanStatus } from "@/types/ipc";

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
    canStart: (s) => !["running", "preparing", "paused"].includes(s.status),
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
          this.refreshDevicesSafe();
          break;
        case "finished":
          this.status = "finished";
          this.addLog(
            "info",
            `finished: done=${e.total} found=${e.found} errors=${e.errors} in ${e.elapsed_ms}ms`
          );
          break;
      }
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

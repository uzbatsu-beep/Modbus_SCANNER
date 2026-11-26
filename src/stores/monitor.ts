/** Incremental monitor state; backend history remains authoritative. */
import { defineStore } from "pinia";
import { api } from "@/api/mod";
import type { CommunicationSnapshot, LoggerConfig, LoggerStatus, MonitorUpdate, MonitorValueState, MonitorWatch } from "@/types/ipc";
import { mergeChannel } from "@/types/measurement";

export const useMonitorStore = defineStore("monitor", {
  state: () => ({
    values: [] as MonitorValueState[], running: false, bound: false, metrics: null as CommunicationSnapshot | null,
    sessionId: null as string | null, refreshing: false, busy: false,
    error: null as string | null, loggers: [] as LoggerStatus[],
  }),
  getters: {
    count: s => s.values.length,
    byKey: s => new Map(s.values.map(v => [v.key, v])),
  },
  actions: {
    async bind() {
      if (this.bound) return;
      this.bound = true;
      try {
        await api.monitor.onUpdate((u: MonitorUpdate) => {
          if (this.sessionId !== u.session_id) return;
          const map = new Map(this.values.map(v => [v.key, v]));
          let gap = false;
          for (const v of u.values) {
            const old = map.get(v.key);
            if (v.samples.length && v.samples[0].sequence > (old?.latest?.sequence ?? 0) + 1) gap = true;
            map.set(v.key, mergeChannel(old, v));
          }
          this.values = [...map.values()];
          this.metrics = u.metrics;
          this.running = u.running;
          if (gap) void this.refresh();
        });
      } catch (e) { this.bound = false; this.error = String(e); }
    },
    async refresh() {
      if (this.refreshing) return;
      this.refreshing = true;
      const session = this.sessionId;
      try {
        const snapshot = await api.scanner.getMonitorState();
        if (session !== this.sessionId) return;
        if (!snapshot.length) return;
        if (session && snapshot[0].session_id !== session) return;
        this.sessionId = snapshot[0].session_id;
        const map = new Map(this.values.map(v => [v.key, v]));
        this.values = snapshot.map(v => mergeChannel(map.get(v.key), v));
      } catch (e) { this.error = String(e); }
      finally { this.refreshing = false; }
    },
    async start(watches: MonitorWatch[], intervalMs: number, historyCapacity = 10000) {
      if (this.busy) return;
      this.busy = true;
      this.error = null;
      try {
        await this.bind();
        if (!this.bound) return;
        this.sessionId = await api.scanner.startMonitor(watches, intervalMs, historyCapacity);
        this.values = [];
        this.metrics = null;
        this.running = true;
        await this.refresh();
      } catch (e) { this.error = String(e); }
      finally { this.busy = false; }
    },
    async refreshLoggers() { this.loggers = await api.scanner.getLoggers(); },
    async setLoggers(configs: LoggerConfig[]) { try { this.loggers = await api.scanner.setLoggers(configs); } catch (e) { this.error = String(e); } },
    async stop() {
      if (this.busy) return;
      this.busy = true;
      try {
        await api.scanner.stopMonitor();
        this.running = false;
        await this.refresh();
        await this.refreshLoggers();
      } catch (e) { this.error = String(e); }
      finally { this.busy = false; }
    },
  },
});

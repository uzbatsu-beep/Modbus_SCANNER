/** Live register monitor state (P3-05). */

import { defineStore } from "pinia";

import { api } from "@/api/mod";
import type { MonitorUpdate, MonitorValueState, MonitorWatch } from "@/types/ipc";

export const useMonitorStore = defineStore("monitor", {
  state: () => ({
    values: [] as MonitorValueState[],
    running: false,
    bound: false,
    lastCycleMs: 0,
    error: null as string | null,
  }),

  getters: {
    count: (s) => s.values.length,
    byKey: (s) => new Map(s.values.map((v) => [v.key, v])),
  },

  actions: {
    async bind() {
      if (this.bound) return;
      this.bound = true;
      await api.monitor.onUpdate((u: MonitorUpdate) => {
        this.values = u.values;
        this.lastCycleMs = u.cycle_ms;
      });
    },

    async refresh() {
      try {
        this.values = await api.scanner.getMonitorState();
      } catch (e) {
        this.error = String(e);
      }
    },

    async start(watches: MonitorWatch[], intervalMs: number) {
      this.error = null;
      try {
        await api.scanner.startMonitor(watches, intervalMs);
        this.running = true;
      } catch (e) {
        this.error = String(e);
      }
    },

    async stop() {
      try {
        await api.scanner.stopMonitor();
        this.running = false;
      } catch (e) {
        this.error = String(e);
      }
    },
  },
});

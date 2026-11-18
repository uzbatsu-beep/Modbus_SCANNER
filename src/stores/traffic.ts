/** Raw Modbus traffic log store (P3-06). */

import { defineStore } from "pinia";

import { api, onTrafficRecord } from "@/api/mod";
import type { ExportFormat, TrafficRecord } from "@/types/ipc";

export const useTrafficStore = defineStore("traffic", {
  state: () => ({
    records: [] as TrafficRecord[],
    bound: false,
    autoScroll: true,
    error: null as string | null,
  }),

  getters: {
    count: (s) => s.records.length,
  },

  actions: {
    async bind() {
      if (this.bound) return;
      this.bound = true;
      await onTrafficRecord((record) => {
        this.records.push(record);
        if (this.records.length > 2000) this.records.shift();
      });
      await this.refresh();
    },

    async refresh() {
      try {
        this.records = await api.scanner.getTrafficLog();
      } catch (e) {
        this.error = String(e);
      }
    },

    async clear() {
      await api.scanner.clearTrafficLog();
      this.records = [];
    },

    async exportLog(path: string, format: ExportFormat) {
      this.error = null;
      try {
        return await api.scanner.exportTrafficLog(path, format);
      } catch (e) {
        this.error = String(e);
        return 0;
      }
    },
  },
});

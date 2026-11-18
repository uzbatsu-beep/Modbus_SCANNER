/**
 * Devices store: found device records + selection.
 */

import { defineStore } from "pinia";
import { api } from "@/api/mod";
import type { DeviceRecord, IpTargetStatus } from "@/types/ipc";

export const useDevicesStore = defineStore("devices", {
  state: () => ({
    records: [] as DeviceRecord[],
    ipTargets: [] as IpTargetStatus[],
    selectedKey: null as string | null,
    loading: false,
  }),

  getters: {
    selected: (s) => s.records.find((r) => r.key === s.selectedKey) ?? null,
    foundCount: (s) => s.records.length,
  },

  actions: {
    async refresh() {
      this.loading = true;
      try {
        const res = await api.scanner.getResults();
        this.records = res.records;
        this.ipTargets = res.ip_targets ?? [];
        // drop selection if the record disappeared
        if (this.selectedKey && !this.records.some((r) => r.key === this.selectedKey)) {
          this.selectedKey = this.records[0]?.key ?? null;
        }
      } finally {
        this.loading = false;
      }
    },

    select(key: string | null) {
      this.selectedKey = key;
    },

    async clear() {
      await api.scanner.clearResults();
      this.records = [];
      this.ipTargets = [];
      this.selectedKey = null;
    },
  },
});

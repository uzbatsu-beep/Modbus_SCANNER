/**
 * Settings store: persisted app config.
 */

import { defineStore } from "pinia";
import { api } from "@/api/mod";
import type { Config } from "@/types/ipc";

const DEFAULT_CONFIG: Config = {
  language: "ru",
  theme: "dark",
  scan_params: {
    timeout_ms: 800,
    request_gap_ms: 20,
    retries: 1,
    probe_functions: [0x03, 0x04, 0x01, 0x02],
    stop_on_first: false,
    register_plan: [],
    ip_concurrency: 32,
  },
  last_serial: null,
  last_tcp_host: null,
  last_tcp_port: 502,
  last_udp_host: null,
  last_udp_port: 502,
  export_dir: null,
  export_format: "csv",
};

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    config: { ...DEFAULT_CONFIG } as Config,
    loaded: false,
    saving: false,
  }),

  actions: {
    async load() {
      try {
        this.config = await api.config.load();
      } catch {
        this.config = { ...DEFAULT_CONFIG };
      } finally {
        this.loaded = true;
      }
    },

    async save() {
      this.saving = true;
      try {
        await api.config.save(this.config);
      } finally {
        this.saving = false;
      }
    },
  },
});

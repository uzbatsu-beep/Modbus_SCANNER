/**
 * Serial ports store: enumeration + refresh.
 */

import { defineStore } from "pinia";
import { api } from "@/api/mod";
import type { SerialPortInfo } from "@/types/ipc";

export const useSerialStore = defineStore("serial", {
  state: () => ({
    ports: [] as SerialPortInfo[],
    loading: false,
    error: null as string | null,
  }),

  actions: {
    async refresh() {
      this.loading = true;
      this.error = null;
      try {
        this.ports = await api.serial.listPorts();
      } catch (e) {
        this.error = String(e);
        this.ports = [];
      } finally {
        this.loading = false;
      }
    },
  },
});

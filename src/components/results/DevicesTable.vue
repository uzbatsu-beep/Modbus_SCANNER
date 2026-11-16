<script setup lang="ts">
import { computed } from "vue";

import type { DeviceRecord, ProbeResult } from "@/types/ipc";
import { useDevicesStore } from "@/stores/devices";

const devices = useDevicesStore();

const rows = computed(() => devices.records);

function probeTag(r: DeviceRecord, fc: number): { cls: string; text: string } {
  const p = r.probes[fc];
  if (!p) return { cls: "dim", text: "—" };
  const res: ProbeResult = p;
  switch (res.status) {
    case "ok":
      return { cls: "ok", text: `OK ${res.rtt_us / 1000 > 0 ? (res.rtt_us / 1000).toFixed(1) : "0"}ms` };
    case "exception":
      return { cls: "warn", text: `EXC ${res.code.toString(16).padStart(2, "0").toUpperCase()}` };
    case "no_response":
      return { cls: "dim", text: "—" };
    case "transport_error":
      return { cls: "error", text: "ERR" };
  }
}

function time(ms: number): string {
  return new Date(ms).toLocaleTimeString();
}

function select(key: string) {
  devices.select(key);
}
</script>

<template>
  <div class="panel">
    <div class="panel-title">Найденные устройства / Devices ({{ rows.length }})</div>

    <div v-if="rows.length === 0" class="empty">Нет данных. Запустите сканирование.</div>

    <table v-else class="data">
      <thead>
        <tr>
          <th>Транспорт</th>
          <th>Адрес / Endpoint</th>
          <th>Unit</th>
          <th>FC 03</th>
          <th>FC 04</th>
          <th>FC 01</th>
          <th>FC 02</th>
          <th>Регистров</th>
          <th>Перв. ответ</th>
          <th>Посл. ответ</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="r in rows"
          :key="r.key"
          :class="{ selected: r.key === devices.selectedKey }"
          @click="select(r.key)"
        >
          <td><span class="tag dim">{{ r.target.kind.toUpperCase() }}</span></td>
          <td>{{ r.target.endpoint }}<span v-if="r.target.port">:{{ r.target.port }}</span></td>
          <td>{{ r.target.unit_id }}</td>
          <td><span class="tag" :class="probeTag(r, 3).cls">{{ probeTag(r, 3).text }}</span></td>
          <td><span class="tag" :class="probeTag(r, 4).cls">{{ probeTag(r, 4).text }}</span></td>
          <td><span class="tag" :class="probeTag(r, 1).cls">{{ probeTag(r, 1).text }}</span></td>
          <td><span class="tag" :class="probeTag(r, 2).cls">{{ probeTag(r, 2).text }}</span></td>
          <td>{{ Object.keys(r.registers).length }}</td>
          <td>{{ time(r.first_seen_ms) }}</td>
          <td>{{ time(r.last_seen_ms) }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.empty {
  color: var(--fg-dim);
  padding: 20px;
  text-align: center;
}

tbody tr {
  cursor: pointer;
}
</style>

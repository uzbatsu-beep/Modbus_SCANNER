<script setup lang="ts">
import { computed } from "vue";

import type { IpTargetClass } from "@/types/ipc";
import { useDevicesStore } from "@/stores/devices";

const devices = useDevicesStore();

const rows = computed(() => [...devices.ipTargets].sort((a, b) => {
  const rank: Record<IpTargetClass, number> = {
    valid_modbus: 5,
    port_open_no_modbus: 4,
    connection_refused: 3,
    timeout: 2,
    unreachable: 1,
    unknown: 0,
  };
  return rank[b.class] - rank[a.class] || a.endpoint.localeCompare(b.endpoint);
}));

const labels: Record<IpTargetClass, string> = {
  valid_modbus: "валидный Modbus",
  port_open_no_modbus: "порт открыт, не Modbus",
  connection_refused: "отказ в соединении",
  timeout: "таймаут",
  unreachable: "недоступен",
  unknown: "не опрошен",
};

const tagClass: Record<IpTargetClass, string> = {
  valid_modbus: "ok",
  port_open_no_modbus: "warn",
  connection_refused: "error",
  timeout: "dim",
  unreachable: "dim",
  unknown: "dim",
};
</script>

<template>
  <div v-if="rows.length" class="panel">
    <div class="panel-title">Классификация IP-целей / IP targets ({{ rows.length }})</div>
    <table class="data">
      <thead>
        <tr>
          <th>Endpoint</th>
          <th>Порт</th>
          <th>Класс</th>
          <th>Unit ID</th>
          <th>OK / EXC</th>
          <th>Таймауты</th>
          <th>Ошибки</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in rows" :key="`${row.endpoint}:${row.port}`">
          <td>{{ row.endpoint }}</td>
          <td>{{ row.port }}</td>
          <td><span class="tag" :class="tagClass[row.class]">{{ labels[row.class] }}</span></td>
          <td class="dim">{{ row.unit_ids_probed.join(", ") }}</td>
          <td>{{ row.responses_ok }} / {{ row.responses_exception }}</td>
          <td>{{ row.timeouts }}</td>
          <td>{{ row.transport_errors }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.dim {
  color: var(--fg-dim);
}
</style>

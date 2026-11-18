<script setup lang="ts">
import { t } from "@/i18n";
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

function classLabel(kind: IpTargetClass): string {
  return t(
    kind === "valid_modbus"
      ? "classValid"
      : kind === "port_open_no_modbus"
        ? "classOpen"
        : kind === "connection_refused"
          ? "classRefused"
          : kind === "timeout"
            ? "classTimeout"
            : kind === "unreachable"
              ? "classUnreachable"
              : "classUnknown"
  );
}

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
    <div class="panel-title">{{ t("ipTargets") }} ({{ rows.length }})</div>
    <table class="data">
      <thead>
        <tr>
          <th>Endpoint</th>
          <th>{{ t("port") }}</th>
          <th>{{ t("ipClass") }}</th>
          <th>{{ t("unitIds") }}</th>
          <th>OK / EXC</th>
          <th>{{ t("timeouts") }}</th>
          <th>{{ t("transportErrors") }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in rows" :key="`${row.endpoint}:${row.port}`">
          <td>{{ row.endpoint }}</td>
          <td>{{ row.port }}</td>
          <td><span class="tag" :class="tagClass[row.class]">{{ classLabel(row.class) }}</span></td>
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

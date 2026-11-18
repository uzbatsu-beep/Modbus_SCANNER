<script setup lang="ts">
import { t } from "@/i18n";
import { computed, nextTick, ref, watch } from "vue";

import type { ExportFormat } from "@/types/ipc";
import { FC_NAMES } from "@/types/ipc";
import { useTrafficStore } from "@/stores/traffic";

const traffic = useTrafficStore();

const functionFilter = ref<number | null>(null);
const statusFilter = ref<string | null>(null);
const path = ref("modbus-traffic.csv");
const format = ref<ExportFormat>("csv");
const msg = ref<string | null>(null);
const list = ref<HTMLElement | null>(null);

const filtered = computed(() =>
  traffic.records.filter(
    (r) =>
      (functionFilter.value === null || r.function === functionFilter.value) &&
      (statusFilter.value === null || r.status === statusFilter.value)
  )
);

const functions = computed(() =>
  Array.from(new Set(traffic.records.map((r) => r.function).filter((f): f is number => f != null))).sort((a, b) => a - b)
);
const statuses = computed(() =>
  Array.from(new Set(traffic.records.map((r) => r.status))).sort()
);

watch(
  () => traffic.count,
  async () => {
    if (!traffic.autoScroll) return;
    await nextTick();
    if (list.value) list.value.scrollTop = list.value.scrollHeight;
  }
);

function time(ms: number): string {
  const d = new Date(ms);
  return d.toLocaleTimeString() + "." + String(ms % 1000).padStart(3, "0");
}

function fcName(fc: number | null | undefined): string {
  return fc == null ? "—" : (FC_NAMES[fc] ?? `0x${fc.toString(16)}`);
}

function statusClass(status: string): string {
  if (status === "ok") return "ok";
  if (status === "timeout") return "dim";
  return "error";
}

async function exportLog() {
  msg.value = null;
  const count = await traffic.exportLog(path.value.trim(), format.value);
  msg.value = count > 0 ? `${t("recordsExported")}: ${count}` : (traffic.error ?? t("errorPrefix"));
}

async function clear() {
  await traffic.clear();
  msg.value = t("clear") + " " + t("saved");
}
</script>

<template>
  <div class="panel traffic-panel">
    <div class="panel-title">
      {{ t("trafficTitle") }} ({{ filtered.length }} / {{ traffic.count }})
    </div>

    <div class="form-row">
      <label>{{ t("function") }}</label>
      <select v-model.number="functionFilter">
        <option :value="null">{{ t("all") }}</option>
        <option v-for="fc in functions" :key="fc" :value="fc">{{ fcName(fc) }}</option>
      </select>
      <label class="short">{{ t("status") }}</label>
      <select v-model="statusFilter">
        <option :value="null">{{ t("all") }}</option>
        <option v-for="s in statuses" :key="s" :value="s">{{ s }}</option>
      </select>
      <label class="check">
        <input v-model="traffic.autoScroll" type="checkbox" />
        {{ t("autoScroll") }}
      </label>
      <button @click="traffic.refresh()">{{ t("update") }}</button>
      <button @click="clear">{{ t("clear") }}</button>
    </div>

    <div ref="list" class="traffic-list">
      <table class="data">
        <thead>
          <tr>
            <th>#</th><th>Время</th><th>Транспорт</th><th>Адрес</th>
            <th>Функция</th><th>Статус</th><th>RTT</th><th>TX</th><th>RX</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in filtered" :key="r.id">
            <td class="dim">{{ r.id }}</td>
            <td class="mono">{{ time(r.timestamp_ms) }}</td>
            <td class="mono">{{ r.kind.toUpperCase() }}</td>
            <td class="mono">{{ r.endpoint }}:{{ r.unit_id }}</td>
            <td>{{ fcName(r.function) }}</td>
            <td><span class="tag" :class="statusClass(r.status)">{{ r.status }}</span></td>
            <td class="mono">{{ r.rtt_us ? (r.rtt_us / 1000).toFixed(2) + "ms" : "—" }}</td>
            <td class="hex">{{ r.tx_hex }}</td>
            <td class="hex">{{ r.rx_hex ?? "—" }}</td>
          </tr>
        </tbody>
      </table>
      <div v-if="filtered.length === 0" class="empty-hint">{{ t("noTraffic") }}</div>
    </div>

    <div class="form-row export-row">
      <label>{{ t("exportLog") }}</label>
      <input v-model="path" class="wide" />
      <select v-model="format">
        <option value="csv">CSV</option>
        <option value="json">JSON</option>
      </select>
      <button class="primary" @click="exportLog">{{ t("export") }}</button>
      <span v-if="msg" class="msg">{{ msg }}</span>
    </div>
  </div>
</template>

<style scoped>
.traffic-panel {
  display: flex;
  flex-direction: column;
  max-height: 100%;
}

.traffic-list {
  overflow-y: auto;
  flex: 1;
  min-height: 240px;
  max-height: 60vh;
  border: 1px solid var(--border);
  border-radius: 6px;
}

.hex {
  max-width: 420px;
  font-family: var(--mono);
  font-size: 11px;
  color: var(--fg-dim);
  word-break: break-all;
}

.check {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: auto !important;
  font-size: 12px;
}

.short {
  min-width: auto !important;
}

.empty-hint,
.msg {
  color: var(--fg-dim);
  font-size: 12px;
  padding: 12px;
}

.export-row {
  margin-top: 10px;
}
</style>

<script setup lang="ts">
import { t } from "@/i18n";
import { computed, ref } from "vue";

import type { LogFormat, LoggerConfig, MonitorWatch, RotationMode } from "@/types/ipc";
import { useDevicesStore } from "@/stores/devices";
import { useMonitorStore } from "@/stores/monitor";
import { useSettingsStore } from "@/stores/settings";

const devices = useDevicesStore();
const monitor = useMonitorStore();
const settings = useSettingsStore();

const interval = ref(1000);
const selectedAddress = ref<number | null>(null);
const watches = ref<MonitorWatch[]>([]);
const trendKey = ref<string | null>(null);
const windowMs = ref(60_000);
const error = ref<string | null>(null);

// P3-07: file loggers attached to the monitor cycle.
const loggerConfigs = ref<LoggerConfig[]>([]);
const loggerForm = reactiveLoggerForm();

function reactiveLoggerForm() {
  return {
    id: "main",
    path: "modbus-values.csv",
    format: "csv" as LogFormat,
    rotation: "none" as "none" | "daily" | "session" | "size",
    sizeMb: 10,
    allKeys: true,
    enabled: true,
  };
}

function addLogger() {
  const rotation: RotationMode =
    loggerForm.rotation === "size"
      ? { size_mb: loggerForm.sizeMb }
      : loggerForm.rotation;
  const config: LoggerConfig = {
    id: loggerForm.id.trim() || `logger-${loggerConfigs.value.length + 1}`,
    path: loggerForm.path.trim(),
    format: loggerForm.format,
    rotation,
    keys: loggerForm.allKeys ? [] : watches.value.map((w) => w.key),
    enabled: loggerForm.enabled,
  };
  loggerConfigs.value = [
    ...loggerConfigs.value.filter((c) => c.id !== config.id),
    config,
  ];
}

async function applyLoggers() {
  error.value = null;
  await monitor.setLoggers(loggerConfigs.value);
}

function removeLogger(id: string) {
  loggerConfigs.value = loggerConfigs.value.filter((c) => c.id !== id);
}

const available = computed(() => {
  if (!devices.selected) return [];
  return Object.values(devices.selected.registers)
    .flatMap((r) => (r ? [r] : []))
    .sort((a, b) => a.address - b.address);
});

function addWatch() {
  const reg = available.value.find((r) => r.address === selectedAddress.value);
  const dev = devices.selected;
  if (!reg || !dev) return;
  const key = `${dev.key}:${reg.function.toString(16)}:${reg.address.toString(16).padStart(4, "0")}`;
  if (watches.value.some((w) => w.key === key)) return;
  const tag = (settings.config.tags ?? []).find(
    (t) => t.device_key === dev.key && t.address === reg.address
  );
  watches.value.push({
    key,
    target: dev.target,
    function: reg.function,
    address: reg.address,
    quantity: 1,
    name: tag?.name ?? `${dev.target.endpoint}:${reg.address}`,
    unit: tag?.unit ?? null,
  });
}

function removeWatch(key: string) {
  watches.value = watches.value.filter((w) => w.key !== key);
  if (trendKey.value === key) trendKey.value = null;
}

async function start() {
  error.value = null;
  if (watches.value.length === 0) {
    error.value = t("noMonitorPoints");
    return;
  }
  await monitor.start(watches.value, interval.value);
  if (monitor.error) error.value = monitor.error;
  if (!trendKey.value) trendKey.value = watches.value[0].key;
}

async function stop() {
  await monitor.stop();
}

function latest(state: { values: number[] }): string {
  if (state.values.length === 0) return "—";
  return state.values.map((v) => `0x${v.toString(16).padStart(4, "0")}`).join(" ");
}

const trendState = computed(() =>
  monitor.values.find((v) => v.key === trendKey.value) ?? null
);

const trendPoints = computed(() => {
  const state = trendState.value;
  if (!state || state.samples.length === 0) return "";
  const cutoff = Date.now() - windowMs.value;
  const samples = state.samples.filter((s) => s.timestamp_ms >= cutoff);
  if (samples.length === 0) return "";
  const values = samples.map((s) => s.values[0] ?? 0);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const width = 600;
  const height = 120;
  return values
    .map((v, i) => {
      const x = (i / Math.max(1, values.length - 1)) * width;
      const y = height - ((v - min) / span) * height;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
});
</script>

<template>
  <div class="panel">
    <div class="panel-title">{{ t("monitorTitle") }} ({{ monitor.count }})</div>

    <div class="form-row">
      <label>{{ t("monitorRegister") }}</label>
      <select v-model.number="selectedAddress" :disabled="monitor.running">
        <option :value="null">{{ t("select") }}</option>
        <option v-for="r in available" :key="r.address" :value="r.address">
          0x{{ r.address.toString(16).padStart(4, "0") }} =
          {{ r.as_hex.toUpperCase() }} (FC{{ r.function.toString(16).toUpperCase() }})
        </option>
      </select>
      <button :disabled="monitor.running || selectedAddress === null" @click="addWatch">
        {{ t("add") }}
      </button>
      <label class="short">{{ t("period") }}</label>
      <input v-model.number="interval" type="number" min="250" max="10000" step="50" :disabled="monitor.running" />
      <button v-if="!monitor.running" class="primary" :disabled="watches.length === 0" @click="start">
        ▶ {{ t("start") }}
      </button>
      <button v-else class="danger" @click="stop">■ {{ t("stop") }}</button>
      <span v-if="error || monitor.error" class="error-text">{{ error || monitor.error }}</span>
    </div>

    <div v-if="watches.length" class="watch-list">
      <span v-for="w in watches" :key="w.key" class="watch-chip mono">
        {{ w.name || w.key }}
        <button :disabled="monitor.running" @click="removeWatch(w.key)">×</button>
      </span>
    </div>

    <table v-if="monitor.values.length" class="data">
      <thead>
        <tr><th>Point</th><th>{{ t("value") }}</th><th>Min</th><th>Max</th><th>{{ t("change") }}</th><th>{{ t("errors") }}</th></tr>
      </thead>
      <tbody>
        <tr
          v-for="v in monitor.values"
          :key="v.key"
          :class="{ 'changed-row': v.changed }"
          @click="trendKey = v.key"
        >
          <td>
            {{ v.name }}
            <span v-if="v.unit" class="dim">{{ v.unit }}</span>
          </td>
          <td class="mono">{{ latest(v) }}</td>
          <td class="mono">{{ v.min ?? "—" }}</td>
          <td class="mono">{{ v.max ?? "—" }}</td>
          <td>
            <span class="tag" :class="v.changed ? 'warn' : 'dim'">
              {{ v.changed ? t("changed") : t("stable") }}
            </span>
          </td>
          <td>{{ v.errors }}</td>
        </tr>
      </tbody>
    </table>
    <div v-else class="hint">{{ t("noMonitorPoints") }}</div>

    <div class="trend">
      <div class="form-row">
        <label>{{ t("trend") }}</label>
        <select v-model="trendKey">
          <option v-for="v in monitor.values" :key="v.key" :value="v.key">{{ v.name }}</option>
        </select>
        <select v-model.number="windowMs">
          <option :value="60_000">{{ t("min1") }}</option>
          <option :value="300_000">{{ t("min5") }}</option>
          <option :value="3_600_000">{{ t("hour1") }}</option>
        </select>
      </div>
      <svg v-if="trendPoints" viewBox="0 0 600 120" class="chart" preserveAspectRatio="none">
        <polyline :points="trendPoints" fill="none" stroke="var(--accent)" stroke-width="2" />
      </svg>
      <div v-else class="hint">{{ t("noTrend") }}</div>
    </div>

    <div class="loggers">
      <div class="section-title">{{ t("loggers") }}</div>

      <div class="form-row">
        <label>ID</label>
        <input v-model="loggerForm.id" class="num" />
        <label class="short">{{ t("file") }}</label>
        <input v-model="loggerForm.path" class="wide" />
        <label class="short">{{ t("format") }}</label>
        <select v-model="loggerForm.format">
          <option value="csv">CSV</option>
          <option value="tsv">TSV</option>
          <option value="json">JSONL</option>
        </select>
      </div>

      <div class="form-row">
        <label>{{ t("rotation") }}</label>
        <select v-model="loggerForm.rotation">
          <option value="none">{{ t("noRotation") }}</option>
          <option value="daily">{{ t("daily") }}</option>
          <option value="session">{{ t("session") }}</option>
          <option value="size">{{ t("size") }}</option>
        </select>
        <input
          v-if="loggerForm.rotation === 'size'"
          v-model.number="loggerForm.sizeMb"
          type="number"
          min="1"
          class="num"
        />
        <label class="check"><input v-model="loggerForm.allKeys" type="checkbox" /> {{ t("allPoints") }}</label>
        <label class="check"><input v-model="loggerForm.enabled" type="checkbox" /> {{ t("enabled") }}</label>
        <button :disabled="!loggerForm.path.trim()" @click="addLogger">{{ t("addToList") }}</button>
        <button class="primary" :disabled="loggerConfigs.length === 0" @click="applyLoggers">
          {{ t("apply") }}
        </button>
        <button @click="monitor.refreshLoggers()">{{ t("statusRefresh") }}</button>
      </div>

      <div v-if="loggerConfigs.length" class="watch-list">
        <span v-for="c in loggerConfigs" :key="c.id" class="watch-chip mono">
          {{ c.id }} → {{ c.path }} ({{ c.format }})
          <button @click="removeLogger(c.id)">×</button>
        </span>
      </div>

      <table v-if="monitor.loggers.length" class="data">
        <thead>
          <tr><th>ID</th><th>{{ t("file") }}</th><th>{{ t("records") }}</th><th>{{ t("status") }}</th></tr>
        </thead>
        <tbody>
          <tr v-for="s in monitor.loggers" :key="s.id">
            <td>{{ s.id }}</td>
            <td class="mono">{{ s.current_file || s.path }}</td>
            <td>{{ s.records_written }}</td>
            <td>
              <span class="tag" :class="s.error ? 'error' : s.enabled ? 'ok' : 'dim'">
                {{ s.error ?? (s.enabled ? t("active") : t("disabled")) }}
              </span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.form-row {
  flex-wrap: wrap;
}

.short {
  min-width: auto !important;
}

.hint,
.dim {
  color: var(--fg-dim);
  font-size: 11px;
}

.error-text {
  color: var(--error);
  font-size: 12px;
}

.watch-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin: 6px 0 10px;
}

.watch-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 2px 8px;
  font-size: 11px;
}

.watch-chip button {
  padding: 0 4px;
  border: none;
  background: transparent;
}

.changed-row {
  background: rgba(255, 183, 77, 0.12);
}

.trend {
  margin-top: 10px;
  border-top: 1px solid var(--border);
  padding-top: 8px;
}

.chart {
  width: 100%;
  height: 120px;
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: 6px;
}

.loggers {
  margin-top: 12px;
  border-top: 1px solid var(--border);
  padding-top: 8px;
}
</style>

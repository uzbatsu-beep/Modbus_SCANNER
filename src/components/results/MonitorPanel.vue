<script setup lang="ts">
import { t } from "@/i18n";
import { computed, reactive, ref, watch, onMounted, onUnmounted } from "vue";
import { trendSegments } from "@/types/measurement";
import { addressSpace, readFunction, registerKey } from "@/types/registerIdentity";

import type { DecodedValue, LogFormat, LoggerConfig, MonitorValueState, MonitorWatch, RotationMode } from "@/types/ipc";
import { useDevicesStore } from "@/stores/devices";
import { useMonitorStore } from "@/stores/monitor";
import { useSettingsStore } from "@/stores/settings";

const devices = useDevicesStore();
const monitor = useMonitorStore();
const settings = useSettingsStore();

const interval = ref(1000);
const historyCapacity = ref(10000);
const selectedPoint = ref<string | null>(null);
watch(() => devices.selected?.key, () => { selectedPoint.value = null; });
const watches = ref<MonitorWatch[]>([]);
const trendKey = ref<string | null>(null);
const windowMs = ref(60_000);
const error = ref<string | null>(null);

// P3-07: file loggers attached to the monitor cycle.
const loggerConfigs = ref<LoggerConfig[]>([]);
const loggerForm = reactive(reactiveLoggerForm());
const clock = ref(Date.now());
let clockTimer: ReturnType<typeof setInterval> | undefined;
onMounted(() => { clockTimer = setInterval(() => { clock.value = Date.now(); }, 1000); });
onUnmounted(() => { if (clockTimer !== undefined) clearInterval(clockTimer); });

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
  const dev = devices.selected;
  if (!dev) return [];
  const points: { label: string; watch: MonitorWatch }[] = [];
  const raw = Object.values(dev.registers).flatMap(r => r ? [r] : [])
    .sort((a,b) => a.function - b.function || a.address - b.address);
  for (const r of raw) {
    if (!addressSpace(r.function)) continue;
    const label = `FC${String(r.function).padStart(2,"0")} · 0x${r.address.toString(16).padStart(4,"0")} · RAW`;
    points.push({ label, watch: {
      key: "raw:" + JSON.stringify([dev.key, registerKey(r.function, r.address)]),
      target: dev.target, function: r.function, address: r.address, quantity: 1,
      name: `${dev.target.endpoint}/${dev.target.unit_id} · ${label}`, unit: null, tag_id: null, spec: null,
    } });
  }
  for (const tag of settings.config.tags.filter(t => t.source.start.device_key === dev.key)) {
    const { start, quantity } = tag.source;
    const fc = readFunction(start.address_space);
    const label = `${tag.name} · FC${String(fc).padStart(2,"0")} · 0x${start.address.toString(16).padStart(4,"0")} × ${quantity} · ${tag.spec.data_type.toUpperCase()}`;
    points.push({ label, watch: {
      key: "tag:" + tag.id, target: dev.target, function: fc, address: start.address, quantity,
      name: `${dev.target.endpoint}/${dev.target.unit_id} · ${label}`, unit: tag.unit, tag_id: tag.id, spec: { ...tag.spec },
    } });
  }
  return points;
});

function addWatch() {
  const point = available.value.find(p => p.watch.key === selectedPoint.value);
  if (!point || watches.value.some(w => w.key === point.watch.key)) return;
  watches.value.push(point.watch);
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
  await monitor.start(watches.value, interval.value, historyCapacity.value);
  if (monitor.error) error.value = monitor.error;
  if (!trendKey.value) trendKey.value = watches.value[0].key;
}

async function stop() {
  await monitor.stop();
}

function formatValue(value: DecodedValue | null | undefined): string {
  return value == null ? "—" : String(value.value);
}
function latest(state: MonitorValueState): string {
  return formatValue(state.latest?.value ?? state.last_good?.value);
}
function raw(state: MonitorValueState): string {
  return state.latest?.raw?.map(v => "0x" + v.toString(16).padStart(4,"0")).join(" ") ?? "—";
}
function stat(value: number | null): string {
  return value === null ? "—" : Number(value.toPrecision(10)).toString();
}
const trendState = computed(() => monitor.values.find(v => v.key === trendKey.value) ?? null);
const trend = computed(() => {
  const state = trendState.value;
  return state?.latest ? trendSegments(state.samples, state.latest.elapsed_us, windowMs.value) : null;
});
function age(state: MonitorValueState): string {
  return state.last_good ? Math.max(0,(clock.value - state.last_good.timestamp_ms)/1000).toFixed(1) + " s" : "—";
}
</script>

<template>
  <div class="panel">
    <div class="panel-title">{{ t("monitorTitle") }} ({{ monitor.count }})</div>

    <div class="form-row">
      <label>{{ t("monitorRegister") }}</label>
      <select v-model="selectedPoint" :disabled="monitor.running">
        <option :value="null">{{ t("select") }}</option>
        <option v-for="p in available" :key="p.watch.key" :value="p.watch.key">{{ p.label }}</option>
      </select>
      <button :disabled="monitor.running || selectedPoint === null" @click="addWatch">
        {{ t("add") }}
      </button>
      <label class="short">{{ t("period") }}</label>
      <input v-model.number="interval" type="number" min="250" max="10000" step="50" :disabled="monitor.running" />
      <label class="short">{{ t("historyCapacity") }}</label>
      <input v-model.number="historyCapacity" type="number" min="100" max="50000" step="100" :disabled="monitor.running || monitor.busy" />
      <button v-if="!monitor.running" class="primary" :disabled="watches.length === 0 || monitor.busy" @click="start">
        ▶ {{ t("start") }}
      </button>
      <button v-else class="danger" :disabled="monitor.busy" @click="stop">■ {{ t("stop") }}</button>
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
        <tr><th>{{ t("nameCol") }}</th><th>RAW</th><th>{{ t("value") }}</th><th>MIN</th><th>MAX</th><th>AVG</th><th>Δ</th><th>{{ t("qualityLabel") }} / Age</th><th>OK / {{ t("errors") }}</th></tr>
      </thead>
      <tbody>
        <tr v-for="v in monitor.values" :key="v.key" @click="trendKey = v.key">
          <td>{{ v.watch.name || v.key }} <span v-if="v.watch.unit" class="dim">{{ v.watch.unit }}</span></td>
          <td class="mono">{{ raw(v) }}</td>
          <td class="mono" :class="{ dim: v.quality !== 'good' || !monitor.running }">{{ latest(v) }} <span v-if="v.quality !== 'good' && v.last_good">({{ t("lastGood") }})</span></td>
          <td class="mono">{{ stat(v.min) }}</td>
          <td class="mono">{{ stat(v.max) }}</td>
          <td class="mono">{{ stat(v.avg) }}</td>
          <td class="mono">{{ stat(v.delta) }}</td>
          <td><span class="tag" :class="v.quality === 'good' && monitor.running ? 'ok' : v.quality === 'not_sampled' ? 'dim' : 'warn'" :title="v.latest?.detail || ''">{{ !monitor.running && v.quality === "good" ? "stale (stopped)" : v.quality }}{{ v.latest?.exception_code != null ? ' 0x' + v.latest.exception_code.toString(16).padStart(2,'0') : '' }}</span>
            <small v-if="v.quality !== 'good' && v.last_good" class="dim"> {{ new Date(v.last_good.timestamp_ms).toLocaleTimeString() }}</small> <small class="dim">{{ age(v) }}</small></td>
          <td>{{ v.valid_samples }} / {{ v.invalid_samples }}</td>
        </tr>
      </tbody>
    </table>
    <div v-else class="hint">{{ t("noMonitorPoints") }}</div>

    <div class="trend">
      <div class="form-row">
        <label>{{ t("trend") }} · {{ trendState?.watch.unit || trendState?.watch.spec?.data_type || "RAW" }}</label>
        <select v-model="trendKey">
          <option v-for="v in monitor.values" :key="v.key" :value="v.key">{{ v.watch.name || v.key }}</option>
        </select>
        <select v-model.number="windowMs">
          <option :value="60_000">{{ t("min1") }}</option>
          <option :value="300_000">{{ t("min5") }}</option>
          <option :value="3_600_000">{{ t("hour1") }}</option>
        </select>
      </div>
      <div v-if="trend" class="hint">{{ stat(trend.hi) }} / 0 / {{ stat(trend.lo) }} · {{ t("actualTimeAxis") }}</div>
      <svg v-if="trend" viewBox="0 0 600 120" class="chart" preserveAspectRatio="none">
        <line x1="0" x2="600" :y1="trend.zero" :y2="trend.zero" stroke="var(--fg-dim)" stroke-dasharray="4 4" />
        <polyline v-for="(points, i) in trend.segments" :key="i" :points="points" fill="none" stroke="var(--accent)" stroke-width="2" />
        <circle v-for="(points, i) in trend.segments.filter(s => !s.includes(' '))" :key="'dot'+i" :cx="Number(points.split(',')[0])" :cy="Number(points.split(',')[1])" r="2" fill="var(--accent)" />
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
          <tr><th>ID</th><th>{{ t("file") }}</th><th>{{ t("records") }}</th><th>{{ t("loggerQueue") }}</th><th>{{ t("status") }}</th></tr>
        </thead>
        <tbody>
          <tr v-for="s in monitor.loggers" :key="s.id">
            <td>{{ s.id }}</td>
            <td class="mono">{{ s.current_file || s.path }}</td>
            <td>{{ s.records_written }}</td>
            <td>{{ s.pending_records }} / {{ s.dropped_records }} / {{ s.failed_records }}</td>
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

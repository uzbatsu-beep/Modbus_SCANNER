<script setup lang="ts">
import { t } from "@/i18n";
import { computed, reactive, ref, watch, onMounted, onUnmounted } from "vue";
import { trendSegments, rttTrend } from "@/types/measurement";
import { addressSpace, readFunction, registerKey } from "@/types/registerIdentity";

import type { CommunicationSnapshot, DecodedValue, LogFormat, LoggerConfig, MonitorValueState, MonitorWatch, RotationMode } from "@/types/ipc";
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
const diagnosticsMode = ref<"devices" | "channels">("devices");
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
  const current: MonitorWatch[] = [];
  for (const w of watches.value) {
    if (!w.tag_id) { current.push(w); continue; }
    const tag = settings.config.tags.find(t => t.id === w.tag_id);
    if (!tag || readFunction(tag.source.start.address_space) !== w.function ||
        tag.source.start.address !== w.address || tag.source.quantity !== w.quantity) {
      error.value = "Tag changed or removed; remove and re-add the watch: " + w.key;
      return;
    }
    current.push({ ...w, unit: tag.unit, spec: { ...tag.spec } });
  }
  await monitor.start(current, interval.value, historyCapacity.value);
  if (monitor.error) error.value = monitor.error;
  if (!trendKey.value || !watches.value.some(w => w.key === trendKey.value)) trendKey.value = watches.value[0].key;
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
const diagnosticRows = computed(() => monitor.metrics?.[diagnosticsMode.value] ?? []);
const staleKeys = computed(() => new Set(monitor.metrics?.channels.filter(c=>c.stale).map(c=>c.key) ?? []));
function effectiveQuality(state: MonitorValueState): string {
  return state.quality === "good" && (!monitor.running || staleKeys.value.has(state.key)) ? "stale" : state.quality;
}
function ms(us: number | null | undefined): string { return us == null ? "—" : (us/1000).toFixed(2); }
function rate(ok: number, total: number): string { return total ? (ok*100/total).toFixed(1)+"%" : "—"; }
function hz(us: number | null): string { return us && us > 0 ? (1000000/us).toFixed(2) : "—"; }
const rttGraph = computed(() => {const s=trendState.value;return s?.latest ? rttTrend(s.samples,s.latest.elapsed_us,windowMs.value) : null;});
const timing = computed(() => trendState.value?.latest?.timing);
const diagnostics = computed<CommunicationSnapshot | null>(() => monitor.metrics);
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
      <label class="short">{{ t("pollDelay") }}</label>
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
          <td class="mono" :class="{ dim: effectiveQuality(v) !== 'good' }">{{ latest(v) }} <span v-if="(effectiveQuality(v) !== 'good') && v.last_good">({{ t("lastGood") }})</span></td>
          <td class="mono">{{ stat(v.min) }}</td>
          <td class="mono">{{ stat(v.max) }}</td>
          <td class="mono">{{ stat(v.avg) }}</td>
          <td class="mono">{{ stat(v.delta) }}</td>
          <td><span class="tag" :class="effectiveQuality(v) === 'good' ? 'ok' : v.quality === 'not_sampled' ? 'dim' : 'warn'" :title="v.latest?.detail || ''">{{ effectiveQuality(v) }}{{ v.latest?.exception_code != null ? ' 0x' + v.latest.exception_code.toString(16).padStart(2,'0') : '' }}</span>
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

    <div v-if="diagnostics" class="comm-diag">
      <div class="section-title">{{ t("communicationDiagnostics") }}</div>
      <div class="metric-cards">
        <div><small>{{ t("readSuccess") }}</small><strong>{{ rate(diagnostics.session.reads_ok,diagnostics.session.attempts) }}</strong></div>
        <div><small>RTT AVG / P95 (ms)</small><strong>{{ ms(diagnostics.session.rtt_avg_us) }} / {{ ms(diagnostics.session.rtt_p95_us) }}</strong></div>
        <div><small>{{ t("cycleDuration") }} / {{ t("cyclePeriod") }} (ms)</small><strong>{{ ms(diagnostics.cycle_us) }} / {{ ms(diagnostics.cycle_period_us) }}</strong></div>
        <div><small>{{ t("expectedInterval") }} (ms)</small><strong>{{ ms(diagnostics.expected_interval_us) }}</strong></div>
        <div><small>TX / RX (ADU)</small><strong>{{ diagnostics.session.requests }} / {{ diagnostics.session.responses }}</strong><small>{{ diagnostics.session.tx_bytes }} / {{ diagnostics.session.rx_bytes }} B</small></div>
        <div><small>{{ t("errors") }}: Timeout / Exception / Invalid / I/O / Decode</small><strong>{{ diagnostics.session.timeouts }} / {{ diagnostics.session.exceptions }} / {{ diagnostics.session.invalid_responses }} / {{ diagnostics.session.transport_errors }} / {{ diagnostics.session.decode_errors }}</strong></div>
      </div>
      <div class="hint">RTT: {{ diagnostics.session.rtt_count }} / 1000 · P50 {{ ms(diagnostics.session.rtt_p50_us) }} · P99 {{ ms(diagnostics.session.rtt_p99_us) }} ms</div>
      <div class="hint">{{ t("exceptionCodes") }}: {{ JSON.stringify(diagnostics.session.exception_codes) }} · {{ t("failureStages") }}: {{ JSON.stringify(diagnostics.session.failure_stages) }}</div>
      <div class="form-row">
        <label>{{ t("communicationDiagnostics") }}</label>
        <select v-model="diagnosticsMode">
          <option value="devices">{{ t("diagDevices") }}</option>
          <option value="channels">{{ t("diagChannels") }}</option>
        </select>
      </div>
      <div class="metric-scroll"><table class="data">
        <thead><tr><th>{{ t("nameCol") }}</th><th>TX/RX</th><th>Read OK</th><th>RTT MIN/MAX</th><th>AVG/P50/P95/P99 (ms)</th><th>{{ t("sampleInterval") }} (ms)</th><th>Hz</th><th>Jitter (ms)</th><th>Errors*</th></tr></thead>
        <tbody><tr v-for="r in diagnosticRows" :key="r.key">
          <td>{{ r.label }} <span v-if="r.stale" class="tag warn">Stale</span></td><td>{{ r.requests }}/{{ r.responses }}</td><td>{{ rate(r.reads_ok,r.attempts) }}</td>
          <td>{{ ms(r.rtt_min_us) }}/{{ ms(r.rtt_max_us) }}</td><td>{{ ms(r.rtt_avg_us) }}/{{ ms(r.rtt_p50_us) }}/{{ ms(r.rtt_p95_us) }}/{{ ms(r.rtt_p99_us) }}</td>
          <td>{{ ms(r.interval_avg_us) }}</td><td>{{ hz(r.interval_avg_us) }}</td><td>{{ ms(r.jitter_avg_us) }}</td><td>{{ r.timeouts }}/{{ r.exceptions }}/{{ r.invalid_responses }}/{{ r.transport_errors }}/{{ r.decode_errors }}</td>
        </tr></tbody>
      </table></div>
      <div class="hint">* Timeout / Exception / Invalid / I/O / Decode</div>
      <div class="section-title">RTT · {{ trendState?.watch.name || trendKey || "—" }} (ms)</div>
      <div v-if="rttGraph" class="hint">0 – {{ ms(rttGraph.hi*1000) }} ms · {{ t("actualTimeAxis") }}</div>
      <svg v-if="rttGraph" viewBox="0 0 600 120" class="chart" preserveAspectRatio="none">
        <polyline v-for="(points,i) in rttGraph.segments" :key="i" :points="points" fill="none" stroke="var(--accent)" stroke-width="2"/>
        <circle v-for="(points,i) in rttGraph.segments.filter(s=>!s.includes(' '))" :key="'rtt'+i" :cx="Number(points.split(',')[0])" :cy="Number(points.split(',')[1])" r="2" fill="var(--accent)"/>
      </svg>
      <div v-else class="hint">{{ t("noTrend") }}</div>
      <div v-if="timing" class="hint">Connect {{ ms(timing.connect_us) }} · Reset {{ ms(timing.reset_us) }} · TX {{ ms(timing.tx_us) }} · RX {{ ms(timing.rx_us) }} · RTT {{ ms(timing.rtt_us) }} · Validation {{ ms(timing.validation_us) }} · Decode {{ ms(timing.decode_us) }} · Total {{ ms(timing.acquisition_us) }} ms · {{ timing.failure_stage || "—" }}</div>
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
        <button class="primary" :disabled="monitor.running || monitor.busy" @click="applyLoggers">
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

<style scoped>
.comm-diag { margin-top: 16px; }
.metric-cards { display: grid; grid-template-columns: repeat(auto-fit,minmax(185px,1fr)); gap: 8px; margin: 8px 0; }
.metric-cards > div { display:flex; flex-direction:column; gap:4px; border:1px solid var(--border, #64748b); border-radius:4px; padding:8px; }
.metric-cards strong { font-variant-numeric:tabular-nums; font-size:1.1rem; }
.metric-scroll {overflow-x:auto;}
</style>

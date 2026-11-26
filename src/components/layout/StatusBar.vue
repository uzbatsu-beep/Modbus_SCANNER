<script setup lang="ts">
import { computed,ref,onMounted } from "vue";
import { api } from "@/api/mod";
import {useMonitorStore} from "@/stores/monitor";
import {useDevicesStore} from "@/stores/devices";
import {useWorkbenchStore} from "@/stores/workbench";

import { t } from "@/i18n";
import { useScannerStore } from "@/stores/scanner";
import type { ScanStatus } from "@/types/ipc";

const scanner = useScannerStore();
const monitor=useMonitorStore(),devices=useDevicesStore(),wb=useWorkbenchStore();
const version=ref<string|null>(null);
onMounted(async()=>{try{version.value=(await api.system.getAppInfo()).version;}catch(e){scanner.addLog('error',String(e));}});
function ms(v:number|null|undefined){return v==null?'—':(v/1000).toFixed(1)+' ms';}

const statusLabel = computed<Record<ScanStatus, string>>(() => ({
  idle: t("statusIdle"),
  preparing: t("statusPreparing"),
  running: t("statusRunning"),
  paused: t("statusPaused"),
  stopping: t("statusStopping"),
  stopped: t("statusStopped"),
  finished: t("statusFinished"),
  failed: t("statusFailed"),
}));

const found = computed(() => scanner.progress?.found ?? 0);
const errors = computed(() => scanner.progress?.errors ?? 0);
const elapsed = computed(() => (scanner.progress?.elapsed_ms ?? 0) / 1000);
const done = computed(() => scanner.progress?.done ?? 0);
const total = computed(() => scanner.progress?.total ?? 0);
// Register-plan work is not part of the strategy's initial `total`, so cap it.
const pct = computed(() =>
  total.value > 0 ? Math.min(100, Math.round((done.value / total.value) * 100)) : 0
);
</script>

<template>
  <footer class="status-bar">
    <span class="status-item">
      <span class="status-dot" :class="scanner.status" />
      {{ statusLabel[scanner.status] }}
    </span>

    <template v-if="scanner.status !== 'idle'">
      <span class="status-item">{{ t("found") }}: {{ found }}</span>
      <span class="status-item">{{ t("errors") }}: {{ errors }}</span>
      <span class="status-item">{{ t("time") }}: {{ elapsed.toFixed(1) }}s</span>
      <span v-if="total > 0" class="status-item">{{ done }}/{{ total }} ({{ pct }}%)</span>
      <div class="progress-track">
        <div class="progress-fill" :style="{ width: pct + '%' }" />
      </div>
    </template>

    <span class="status-item">{{ t("monitorTitle") }}: {{ monitor.running?t("wbRunning"):t("statusIdle") }} · {{ monitor.count }}</span>
    <span v-if="monitor.metrics && monitor.running" class="status-item">RTT {{ ms(monitor.metrics.session.rtt_avg_us) }} · {{ t("cyclePeriod") }} {{ ms(monitor.metrics.cycle_period_us) }} · {{ t("errors") }} {{ monitor.metrics.session.timeouts + monitor.metrics.session.exceptions + monitor.metrics.session.invalid_responses + monitor.metrics.session.transport_errors + monitor.metrics.session.decode_errors }}</span>
    <span v-if="devices.selected" class="status-item">{{ devices.selected.target.kind.toUpperCase() }} · {{ devices.selected.target.endpoint }} / {{ devices.selected.target.unit_id }}</span>
    <span v-if="wb.storageError" role="alert" class="status-item">{{ t("wbLayoutSaveError") }}</span>
    <span v-if="monitor.error" :title="monitor.error" class="status-item">{{ t("errors") }}: {{ monitor.error }}</span>
    <span class="wb-spacer"/><span class="status-item dim">Modbus Scanner {{ version ? "v"+version : "—" }}</span>
  </footer>
</template>

<style scoped>
.status-item {
  white-space: nowrap;
}

.status-item.dim {
  color: var(--fg-dim);
}
</style>

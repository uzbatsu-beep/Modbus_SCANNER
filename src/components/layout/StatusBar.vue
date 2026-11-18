<script setup lang="ts">
import { computed } from "vue";

import { t } from "@/i18n";
import { useScannerStore } from "@/stores/scanner";
import type { ScanStatus } from "@/types/ipc";

const scanner = useScannerStore();

const statusLabel = computed<Record<ScanStatus, string>>(() => ({
  idle: t("statusIdle"),
  preparing: t("statusPreparing"),
  running: t("statusRunning"),
  paused: t("statusPaused"),
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

    <span v-else class="status-item dim">Modbus Scanner v0.1.0</span>
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

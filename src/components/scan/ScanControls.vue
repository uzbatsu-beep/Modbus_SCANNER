<script setup lang="ts">
import { t } from "@/i18n";
import { computed } from "vue";

import { useScannerStore } from "@/stores/scanner";
import { useDevicesStore } from "@/stores/devices";

const scanner = useScannerStore();
const devices = useDevicesStore();

const statusText = computed<Record<string, string>>(() => ({
  idle: t("statusIdle"),
  preparing: t("statusPreparing"),
  running: t("statusRunning"),
  paused: t("statusPaused"),
  stopped: t("statusStopped"),
  finished: t("statusFinished"),
  failed: t("startFailed"),
}));
</script>

<template>
  <div class="panel controls">
    <div class="controls-left">
      <button v-if="scanner.canStart" class="primary" disabled>▶ {{ t("start") }}</button>
      <button v-if="scanner.isRunning" class="danger" @click="scanner.stop()">■ {{ t("stop") }}</button>
      <button v-if="scanner.isRunning" class="warn" @click="scanner.pause()">⏸ {{ t("pause") }}</button>
      <button v-if="scanner.isPaused" class="warn" @click="scanner.resume()">▶ {{ t("resume") }}</button>
      <button v-if="!scanner.canStart && !scanner.isRunning" class="danger" @click="scanner.stop()">
        ■ {{ t("stop") }}
      </button>
      <span class="status">{{ statusText[scanner.status] }}</span>
    </div>
    <div class="controls-right">
      <span class="found">{{ t("devicesFound") }}: {{ devices.foundCount }}</span>
      <button v-if="devices.foundCount > 0 && scanner.canStart" @click="devices.clear()">
        {{ t("clearResults") }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.controls {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.controls-left,
.controls-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.status {
  font-family: var(--mono);
  font-size: 12px;
  color: var(--fg-dim);
}

.found {
  font-family: var(--mono);
  font-size: 12px;
  color: var(--ok);
}
</style>

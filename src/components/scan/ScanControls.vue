<script setup lang="ts">
import { computed } from "vue";

import { useScannerStore } from "@/stores/scanner";
import { useDevicesStore } from "@/stores/devices";

const scanner = useScannerStore();
const devices = useDevicesStore();

const statusText = computed<Record<string, string>>(() => ({
  idle: "ожидание",
  preparing: "подготовка",
  running: "сканирование...",
  paused: "пауза",
  stopped: "остановлено",
  finished: "завершено",
  failed: "ошибка запуска",
}));
</script>

<template>
  <div class="panel controls">
    <div class="controls-left">
      <button v-if="scanner.canStart" class="primary" disabled>▶ Старт</button>
      <button v-if="scanner.isRunning" class="danger" @click="scanner.stop()">■ Стоп</button>
      <button v-if="scanner.isRunning" class="warn" @click="scanner.pause()">⏸ Пауза</button>
      <button v-if="scanner.isPaused" class="warn" @click="scanner.resume()">▶ Продолжить</button>
      <button v-if="!scanner.canStart && !scanner.isRunning" class="danger" @click="scanner.stop()">
        ■ Стоп
      </button>
      <span class="status">{{ statusText[scanner.status] }}</span>
    </div>
    <div class="controls-right">
      <span class="found">найдено устройств: {{ devices.foundCount }}</span>
      <button v-if="devices.foundCount > 0 && scanner.canStart" @click="devices.clear()">
        Очистить результаты
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

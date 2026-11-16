<script setup lang="ts">
import { computed, reactive } from "vue";

import type { ScanParams, ScanTarget } from "@/types/ipc";
import { useScannerStore } from "@/stores/scanner";
import { useSettingsStore } from "@/stores/settings";

const props = defineProps<{ params: ScanParams }>();
const emit = defineEmits<{ (e: "start", target: ScanTarget): void }>();

const scanner = useScannerStore();
const settings = useSettingsStore();

const form = reactive({
  kind: "tcp" as "tcp" | "udp",
  from: settings.config.last_tcp_host ?? "192.168.1.1",
  to: "" as string,
  port: settings.config.last_tcp_port ?? 502,
});

const canStart = computed(
  () => form.from.length > 0 && form.port > 0 && form.port < 65536 && scanner.canStart
);

function start() {
  const target: ScanTarget = {
    type: "ip_range",
    from: form.from,
    to: form.to || null,
    port: form.port,
    kind: form.kind,
  };
  emit("start", target);
}
</script>

<template>
  <div class="panel">
    <div class="panel-title">Modbus TCP / UDP — сканер сети (IP range)</div>

    <div class="form-row">
      <label>Транспорт</label>
      <select v-model="form.kind" :disabled="scanner.isRunning">
        <option value="tcp">TCP (MBAP)</option>
        <option value="udp">UDP (RTU-over-UDP)</option>
      </select>
    </div>

    <div class="form-row">
      <label>IP: от</label>
      <input v-model="form.from" placeholder="192.168.1.1 или 192.168.1.0/24" :disabled="scanner.isRunning" />
      <label class="short">до</label>
      <input v-model="form.to" placeholder="192.168.1.255 (пусто = один/подсеть)" :disabled="scanner.isRunning" />
    </div>

    <div class="form-row">
      <label>Порт</label>
      <input v-model.number="form.port" type="number" min="1" max="65535" :disabled="scanner.isRunning" />
      <span class="hint">Modbus по стандарту: 502; часто 503, 1502, 802</span>
    </div>

    <div class="form-row">
      <label>Примечание</label>
      <span class="hint">
        TCP: unit id 1 и 255 · UDP: 1 запрос = 1 датаграмма, таймаут = нет устройства
      </span>
    </div>

    <div class="form-row">
      <label />
      <button class="primary" :disabled="!canStart" @click="start">
        ▶ Сканировать сеть
      </button>
    </div>
  </div>
</template>

<style scoped>
.short {
  min-width: auto !important;
}

.hint {
  font-size: 11px;
  color: var(--fg-dim);
}
</style>

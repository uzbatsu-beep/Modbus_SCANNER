<script setup lang="ts">
import { computed, reactive } from "vue";

import type { ManualTarget, RegisterScanMode, ScanTarget } from "@/types/ipc";
import { FC_NAMES } from "@/types/ipc";
import { useDevicesStore } from "@/stores/devices";
import { useScannerStore } from "@/stores/scanner";

const emit = defineEmits<{ (e: "start", target: ScanTarget): void }>();

const scanner = useScannerStore();
const devices = useDevicesStore();

const form = reactive({
  kind: "tcp" as "rtu" | "ascii" | "tcp" | "udp",
  endpoint: "192.168.1.1",
  port: 502,
  unit_id: 1,
  serial_port: "COM3",
  baud_rate: 9600,
  function: 0x03,
  start: 0,
  end: 999,
  mode: "balanced" as RegisterScanMode,
  batch: 0,
});

const isSerial = computed(() => form.kind === "rtu" || form.kind === "ascii");
const canStart = computed(
  () =>
    form.endpoint.trim().length > 0 &&
    form.start <= form.end &&
    form.end <= 65535 &&
    scanner.canStart
);

function useSelected() {
  const dev = devices.selected;
  if (!dev) return;
  form.kind = dev.target.kind;
  form.endpoint = dev.target.endpoint;
  form.port = dev.target.port ?? 502;
  form.unit_id = dev.target.unit_id;
  if (dev.target.serial) form.baud_rate = dev.target.serial.baud_rate;
}

function target(): ManualTarget {
  return {
    kind: form.kind,
    endpoint: isSerial.value ? form.serial_port.trim() : form.endpoint.trim(),
    port: isSerial.value ? null : form.port,
    unit_id: form.unit_id,
    serial: isSerial.value
      ? {
          port_name: form.serial_port.trim(),
          baud_rate: form.baud_rate,
          data_bits: "eight",
          parity: "none",
          stop_bits: "one",
        }
      : null,
    serial_mode: isSerial.value
      ? form.kind === "ascii"
        ? "ascii"
        : "rtu"
      : null,
  };
}

function start() {
  emit("start", {
    type: "register_scan",
    scan: {
      target: target(),
      function: form.function,
      start: form.start,
      end: form.end,
      mode: form.mode,
      batch: form.batch,
    },
  });
}
</script>

<template>
  <div class="panel">
    <div class="panel-title">Карта регистров / Register map (адаптивный скан)</div>

    <div class="form-row">
      <label>Транспорт</label>
      <select v-model="form.kind" :disabled="scanner.isRunning">
        <option value="tcp">TCP</option>
        <option value="udp">UDP</option>
        <option value="rtu">RTU (serial)</option>
        <option value="ascii">ASCII (serial)</option>
      </select>
      <label class="short">Unit ID</label>
      <input v-model.number="form.unit_id" type="number" min="0" max="255" :disabled="scanner.isRunning" />
      <button v-if="devices.selected" :disabled="scanner.isRunning" @click="useSelected">
        ← из выбранного устройства
      </button>
    </div>

    <div v-if="!isSerial" class="form-row">
      <label>Хост</label>
      <input v-model="form.endpoint" placeholder="192.168.1.1" :disabled="scanner.isRunning" />
      <label class="short">Порт</label>
      <input v-model.number="form.port" type="number" min="1" max="65535" :disabled="scanner.isRunning" />
    </div>

    <div v-else class="form-row">
      <label>Порт / Port</label>
      <input v-model="form.serial_port" placeholder="COM3" :disabled="scanner.isRunning" />
      <label class="short">Baud</label>
      <input v-model.number="form.baud_rate" type="number" :disabled="scanner.isRunning" />
    </div>

    <div class="form-row">
      <label>Функция</label>
      <select v-model.number="form.function" :disabled="scanner.isRunning">
        <option :value="0x01">{{ FC_NAMES[0x01] }}</option>
        <option :value="0x02">{{ FC_NAMES[0x02] }}</option>
        <option :value="0x03">{{ FC_NAMES[0x03] }}</option>
        <option :value="0x04">{{ FC_NAMES[0x04] }}</option>
      </select>
      <label class="short">Адрес от</label>
      <input v-model.number="form.start" type="number" min="0" max="65535" :disabled="scanner.isRunning" />
      <label class="short">до</label>
      <input v-model.number="form.end" type="number" min="0" max="65535" :disabled="scanner.isRunning" />
    </div>

    <div class="form-row">
      <label>Стратегия</label>
      <select v-model="form.mode" :disabled="scanner.isRunning">
        <option value="quick">Quick — блоки, один сплит на 1/5</option>
        <option value="balanced">Balanced — 100 → 20 → 5 → регистры</option>
        <option value="deep">Deep — до одиночных регистров</option>
      </select>
      <label class="short">Размер блока (0 = авто)</label>
      <input v-model.number="form.batch" type="number" min="0" max="2000" :disabled="scanner.isRunning" />
    </div>

    <div class="form-row">
      <label />
      <button class="primary" :disabled="!canStart" @click="start">
        ▶ Сканировать {{ form.end - form.start + 1 }} адресов
      </button>
      <span class="hint">
        Timeouts не делятся; 3 подряд → стоп. Exception 01/03 → немедленный стоп.
      </span>
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

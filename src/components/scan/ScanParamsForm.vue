<script setup lang="ts">
import type { ScanParams } from "@/types/ipc";
import { FC, FC_NAMES } from "@/types/ipc";
import { useScannerStore } from "@/stores/scanner";

const params = defineModel<ScanParams>({ required: true });

const scanner = useScannerStore();

const READ_FCS = [FC.READ_COILS, FC.READ_DISCRETE_INPUTS, FC.READ_HOLDING_REGISTERS, FC.READ_INPUT_REGISTERS];

function toggleProbe(fc: number) {
  const i = params.value.probe_functions.indexOf(fc);
  if (i >= 0) params.value.probe_functions.splice(i, 1);
  else params.value.probe_functions.push(fc);
}
</script>

<template>
  <div class="panel">
    <div class="panel-title">Параметры сканирования / Scan params</div>

    <div class="form-row">
      <label>Таймаут ответа, мс</label>
      <input v-model.number="params.timeout_ms" type="number" min="50" max="10000" :disabled="scanner.isRunning" />
      <label class="short">Пауза между req, мс</label>
      <input v-model.number="params.request_gap_ms" type="number" min="0" max="1000" :disabled="scanner.isRunning" />
    </div>

    <div class="form-row">
      <label>Ретраев (до «нет ответа»)</label>
      <input v-model.number="params.retries" type="number" min="0" max="5" :disabled="scanner.isRunning" />
      <label class="short">Параллелизм IP</label>
      <input v-model.number="params.ip_concurrency" type="number" min="1" max="256" :disabled="scanner.isRunning" />
    </div>

    <div class="form-row">
      <label>FC зонда устройства</label>
      <label v-for="fc in READ_FCS" :key="fc" class="check">
        <input
          type="checkbox"
          :checked="params.probe_functions.includes(fc)"
          :disabled="scanner.isRunning"
          @change="toggleProbe(fc)"
        />
        {{ FC_NAMES[fc] }}
      </label>
    </div>

    <div class="form-row">
      <label>Стоп после 1-го найден.</label>
      <input type="checkbox" v-model="params.stop_on_first" :disabled="scanner.isRunning" />
      <label class="short">FC43 идентификация</label>
      <input type="checkbox" v-model="params.identify_devices" :disabled="scanner.isRunning" />
      <span class="hint">запрос Vendor / Product / Revision после ответа устройства</span>
    </div>

    <div class="hint">
      Рекомендации: RTU @9600 → timeout 500–1000 мс; шина с 10+ устройствами → gap 20–50 мс;
      IP → timeout 200–500 мс, concurrency 32–64.
    </div>
  </div>
</template>

<style scoped>
.short {
  min-width: auto !important;
}

.check {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: auto !important;
  font-size: 12px;
  cursor: pointer;
}

.hint {
  font-size: 11px;
  color: var(--fg-dim);
  margin-top: 6px;
}
</style>

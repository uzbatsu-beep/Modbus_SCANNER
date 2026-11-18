<script setup lang="ts">
import { computed, ref, watch } from "vue";

import { api } from "@/api/mod";
import type { ExportFormat } from "@/types/ipc";
import { FC_NAMES } from "@/types/ipc";
import { useDevicesStore } from "@/stores/devices";
import RegisterInspector from "@/components/results/RegisterInspector.vue";

const devices = useDevicesStore();

const objectNames: Record<number, string> = {
  0: "VendorName",
  1: "ProductCode",
  2: "MajorMinorRevision",
  3: "VendorUrl",
  4: "ProductName",
  5: "ModelName",
  6: "UserAppName",
};

const fmt = ref<ExportFormat>("csv");
const exporting = ref(false);
const exportMsg = ref<string | null>(null);

const dev = computed(() => devices.selected);

// Up to two registers can be selected for the interactive decoder.
const selectedAddresses = ref<number[]>([]);
watch(
  () => dev.value?.key,
  () => {
    selectedAddresses.value = [];
  }
);

function toggleAddress(address: number) {
  const i = selectedAddresses.value.indexOf(address);
  if (i >= 0) {
    selectedAddresses.value.splice(i, 1);
    return;
  }
  if (selectedAddresses.value.length >= 2) selectedAddresses.value.shift();
  selectedAddresses.value.push(address);
}

const selectedValues = computed(() => {
  if (!dev.value) return [];
  const byAddress = new Map(
    Object.values(dev.value.registers).map((r) => [r.address, r.raw])
  );
  return selectedAddresses.value
    .slice()
    .sort((a, b) => a - b)
    .map((a) => byAddress.get(a) ?? 0);
});

const registers = computed(() => {
  if (!dev.value) return [];
  return Object.values(dev.value.registers)
    .sort((a, b) => a.function - b.function || a.address - b.address);
});

const probes = computed(() => {
  if (!dev.value) return [];
  return Object.entries(dev.value.probes).map(([fc, p]) => ({ fc: Number(fc), p }));
});

async function exportResults() {
  if (!dev.value) return;
  exporting.value = true;
  exportMsg.value = null;
  try {
    const safeKey = dev.value.key.replace(/[^a-zA-Z0-9_-]+/g, "_");
    const defaultName = `modbus-device-${safeKey}-${new Date().toISOString().replace(/[:.]/g, "-")}.${fmt.value}`;
    const path = prompt("Путь для сохранения файла:", defaultName);
    if (!path) return;
    await api.scanner.exportDevice(dev.value.key, path, fmt.value);
    exportMsg.value = `устройство экспортировано: ${dev.value.key}`;
  } catch (e) {
    exportMsg.value = `ошибка: ${e}`;
  } finally {
    exporting.value = false;
  }
}
</script>

<template>
  <div class="panel">
    <div class="panel-title">Детали устройства / Device detail</div>

    <div v-if="!dev" class="empty">Выберите устройство в таблице</div>

    <template v-else>
      <div class="detail-grid">
        <div><span class="lbl">Ключ:</span> <span class="mono">{{ dev.key }}</span></div>
        <div><span class="lbl">Транспорт:</span> {{ dev.target.kind.toUpperCase() }}</div>
        <div>
          <span class="lbl">Адрес:</span>
          <span class="mono">{{ dev.target.endpoint }}{{ dev.target.port ? ":" + dev.target.port : "" }}</span>
        </div>
        <div><span class="lbl">Unit ID:</span> {{ dev.target.unit_id }}</div>
      </div>

      <div v-if="dev.device_identification" class="section">
        <div class="section-title">Идентификация / Device ID (FC43 / MEI 0x0E)</div>
        <table class="data">
          <thead>
            <tr><th>Объект</th><th>Значение</th></tr>
          </thead>
          <tbody>
            <tr v-for="obj in dev.device_identification.objects" :key="obj.id">
              <td class="obj-id">{{ objectNames[obj.id] ?? `Object ${obj.id}` }}</td>
              <td>{{ obj.value }}</td>
            </tr>
          </tbody>
        </table>
        <div class="hint">
          conformity level: {{ dev.device_identification.conformity_level }} ·
          {{ dev.device_identification.more_follows ? "есть продолжение" : "полный ответ" }}
        </div>
      </div>

      <div class="section">
        <div class="section-title">Результаты зондов / Probes</div>
        <table class="data">
          <thead>
            <tr><th>Функция</th><th>Статус</th><th>Значения / Ошибка</th><th>RTT</th><th>Raw</th></tr>
          </thead>
          <tbody>
            <tr v-for="p in probes" :key="p.fc">
              <td>{{ FC_NAMES[p.fc] ?? "FC " + p.fc.toString(16) }}</td>
              <td>
                <span
                  class="tag"
                  :class="p.p.status === 'ok' ? 'ok' : p.p.status === 'exception' ? 'warn' : p.p.status === 'transport_error' ? 'error' : 'dim'"
                >
                  {{ p.p.status }}
                </span>
              </td>
              <td>
                <template v-if="p.p.status === 'ok'">
                  <span class="mono">{{ p.p.values.map((v) => v.toString(16).padStart(4, "0")).join(" ") }}</span>
                </template>
                <template v-else-if="p.p.status === 'exception'">
                  {{ p.p.message }}
                </template>
                <template v-else-if="p.p.status === 'transport_error'">
                  {{ p.p.message }}
                </template>
              </td>
              <td>{{ p.p.status === "ok" || p.p.status === "exception" ? (p.p.rtt_us / 1000).toFixed(1) + "ms" : "—" }}</td>
              <td><span v-if="p.p.status === 'ok'" class="hex">{{ p.p.raw_hex }}</span></td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="section">
        <div class="section-title">Карта регистров / Register map ({{ registers.length }})</div>
        <div v-if="registers.length === 0" class="empty">Карта регистров не сканировалась</div>
        <table v-else class="data regs">
          <thead>
            <tr><th colspan="2">Выбор</th><th>Адрес</th><th>Hex</th><th>Dec</th><th>Bin</th></tr>
          </thead>
          <tbody>
            <tr v-for="r in registers" :key="`${r.function}:${r.address}`">
              <td>{{ r.function.toString(16).padStart(2, "0").toUpperCase() }}</td>
              <td>
                <input
                  type="checkbox"
                  :checked="selectedAddresses.includes(r.address)"
                  @change="toggleAddress(r.address)"
                />
              </td>
              <td>{{ r.address.toString(16).padStart(4, "0").toUpperCase() }} ({{ r.address }})</td>
              <td>{{ r.as_hex.toUpperCase() }}</td>
              <td>{{ r.raw }}</td>
              <td class="bin">{{ r.raw.toString(2).padStart(16, "0") }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <RegisterInspector
        v-if="registers.length"
        :record="dev"
        :values="selectedValues"
        :addresses="selectedAddresses.slice().sort((a, b) => a - b)"
      />

      <div class="section export">
        <div class="section-title">Экспорт устройства</div>
        <select v-model="fmt">
          <option value="csv">CSV</option>
          <option value="json">JSON</option>
          <option value="xml">XML</option>
        </select>
        <button :disabled="exporting" @click="exportResults()">Экспорт</button>
        <span v-if="exportMsg" class="export-msg">{{ exportMsg }}</span>
      </div>
    </template>
  </div>
</template>

<style scoped>
.empty {
  color: var(--fg-dim);
  padding: 20px;
  text-align: center;
}

.detail-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px 20px;
  margin-bottom: 14px;
  font-size: 13px;
}

.lbl {
  color: var(--fg-dim);
}

.section {
  margin-bottom: 14px;
}

.section-title {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--fg-dim);
  margin-bottom: 6px;
}

.regs td.bin {
  font-size: 11px;
  color: var(--fg-dim);
}

.export {
  display: flex;
  align-items: center;
  gap: 10px;
}

.export-msg {
  font-family: var(--mono);
  font-size: 11px;
  color: var(--fg-dim);
}

.obj-id {
  width: 180px;
  color: var(--fg-dim);
}

.hint {
  font-size: 11px;
  color: var(--fg-dim);
  margin-top: 4px;
}
</style>

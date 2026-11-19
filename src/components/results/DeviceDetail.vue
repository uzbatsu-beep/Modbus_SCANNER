<script setup lang="ts">
import { t } from "@/i18n";
import { computed, ref, watch } from "vue";

import { api } from "@/api/mod";
import type { DeviceProfile, ExportFormat } from "@/types/ipc";
import { FC_NAMES } from "@/types/ipc";
import { useDevicesStore } from "@/stores/devices";
import { useSettingsStore } from "@/stores/settings";
import RegisterInspector from "@/components/results/RegisterInspector.vue";

const devices = useDevicesStore();
const settings = useSettingsStore();

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

const suggestedProfile = ref<DeviceProfile | null>(null);
watch(
  () => dev.value?.key,
  async (key) => {
    suggestedProfile.value = null;
    if (!key || !dev.value?.device_identification) return;
    try {
      suggestedProfile.value = await api.profiles.suggest(key);
    } catch {
      suggestedProfile.value = null;
    }
  },
  { immediate: true }
);

async function applySuggestedProfile() {
  if (!dev.value || !suggestedProfile.value) return;
  await api.profiles.apply(dev.value.key, suggestedProfile.value.id);
  await settings.load();
  await devices.refresh();
}

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
    exportMsg.value = `${t("exportedDevice")}: ${dev.value.key}`;
  } catch (e) {
    exportMsg.value = `ошибка: ${e}`;
  } finally {
    exporting.value = false;
  }
}
</script>

<template>
  <div class="panel">
    <div class="panel-title">{{ t("deviceDetail") }}</div>

    <div v-if="!dev" class="empty">{{ t("selectDevice") }}</div>

    <template v-else>
      <div class="detail-grid">
        <div><span class="lbl">{{ t("key") }}:</span> <span class="mono">{{ dev.key }}</span></div>
        <div><span class="lbl">{{ t("transportCol") }}:</span> {{ dev.target.kind.toUpperCase() }}</div>
        <div>
          <span class="lbl">{{ t("address") }}:</span>
          <span class="mono">{{ dev.target.endpoint }}{{ dev.target.port ? ":" + dev.target.port : "" }}</span>
        </div>
        <div><span class="lbl">Unit ID:</span> {{ dev.target.unit_id }}</div>
      </div>

      <div v-if="dev.device_identification" class="section">
        <div class="section-title">{{ t("identity") }}</div>
        <table class="data">
          <thead>
            <tr><th>{{ t("object") }}</th><th>{{ t("value") }}</th></tr>
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
          {{ dev.device_identification.more_follows ? t("moreFollows") : t("fullAnswer") }}
        </div>
      </div>

      <div v-if="suggestedProfile" class="section">
        <div class="section-title">{{ t("applyProfile") }}: {{ suggestedProfile.name }}</div>
        <button class="primary" @click="applySuggestedProfile">
          {{ t("applyProfile") }} ({{ suggestedProfile.registers.length }} {{ t("registers") }})
        </button>
      </div>

      <div class="section">
        <div class="section-title">{{ t("probes") }}</div>
        <table class="data">
          <thead>
            <tr><th>{{ t("function") }}</th><th>{{ t("status") }}</th><th>{{ t("values") }}</th><th>{{ t("rtt") }}</th><th>{{ t("raw") }}</th></tr>
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
        <div class="section-title">{{ t("registerMap") }} ({{ registers.length }})</div>
        <div v-if="registers.length === 0" class="empty">{{ t("noRegisterMap") }}</div>
        <table v-else class="data regs">
          <thead>
            <tr><th colspan="2">{{ t("select") }}</th><th>{{ t("address") }}</th><th>Hex</th><th>Dec</th><th>Bin</th></tr>
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
        <div class="section-title">{{ t("exportDevice") }}</div>
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

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";

import { api } from "@/api/mod";
import { t } from "@/i18n";
import type { DeviceProfile } from "@/types/ipc";
import { useDevicesStore } from "@/stores/devices";
import { useSettingsStore } from "@/stores/settings";

const devices = useDevicesStore();
const settings = useSettingsStore();

const profiles = ref<DeviceProfile[]>([]);
const selectedId = ref<string | null>(null);
const importPath = ref("profiles.csv");
const importFormat = ref<"csv" | "json">("csv");
const createKey = ref<string | null>(null);
const applyKey = ref<string | null>(null);
const msg = ref<string | null>(null);
const error = ref<string | null>(null);

const selected = computed(() => profiles.value.find((p) => p.id === selectedId.value) ?? null);
const identifiedDevices = computed(() =>
  devices.records.filter((r) => r.device_identification)
);

async function refresh() {
  try {
    profiles.value = await api.profiles.list();
    if (!profiles.value.some((p) => p.id === selectedId.value)) {
      selectedId.value = profiles.value[0]?.id ?? null;
    }
  } catch (e) {
    error.value = String(e);
  }
}

onMounted(async () => {
  await refresh();
  await devices.refresh().catch(() => {});
});

async function importFile() {
  error.value = null;
  msg.value = null;
  try {
    const result = await api.profiles.importFile(importPath.value.trim(), importFormat.value);
    await settings.load();
    await refresh();
    msg.value = `${t("imported")}: ${result.length}`;
  } catch (e) {
    error.value = String(e);
  }
}

async function createFromDevice() {
  error.value = null;
  msg.value = null;
  if (!createKey.value) return;
  try {
    const created = await api.profiles.fromDevice(createKey.value);
    await api.profiles.save([...profiles.value.filter((p) => p.id !== created.id), created]);
    await settings.load();
    await refresh();
    selectedId.value = created.id;
    msg.value = t("created");
  } catch (e) {
    error.value = String(e);
  }
}

async function apply() {
  error.value = null;
  msg.value = null;
  if (!applyKey.value || !selectedId.value) return;
  try {
    const tags = await api.profiles.apply(applyKey.value, selectedId.value);
    await settings.load();
    msg.value = `${t("applied")}: ${tags.length}`;
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <div class="panel">
    <div class="panel-title">{{ t("profilesTitle") }} ({{ profiles.length }})</div>

    <div class="form-row">
      <label>{{ t("importFile") }}</label>
      <input v-model="importPath" class="wide" />
      <select v-model="importFormat">
        <option value="csv">CSV</option>
        <option value="json">JSON</option>
      </select>
      <button :disabled="!importPath.trim()" @click="importFile">{{ t("importFile") }}</button>
      <button @click="refresh">↻</button>
    </div>

    <div class="form-row">
      <label>{{ t("createFromDevice") }}</label>
      <select v-model="createKey">
        <option :value="null">—</option>
        <option v-for="r in identifiedDevices" :key="r.key" :value="r.key">
          {{ r.key }} — {{ r.device_identification?.objects.find((o) => o.id === 0)?.value }}
        </option>
      </select>
      <button :disabled="!createKey" @click="createFromDevice">{{ t("createFromDevice") }}</button>
    </div>

    <div v-if="error" class="error-text">{{ error }}</div>
    <div v-if="msg" class="msg">{{ msg }}</div>

    <div v-if="profiles.length === 0" class="hint">{{ t("noProfiles") }}</div>

    <div v-else class="profiles-grid">
      <table class="data">
        <thead>
          <tr><th>{{ t("nameCol") }}</th><th>{{ t("vendor") }}</th><th>{{ t("productCode") }}</th><th>{{ t("revision") }}</th><th>{{ t("registersCount") }}</th></tr>
        </thead>
        <tbody>
          <tr
            v-for="p in profiles"
            :key="p.id"
            :class="{ selected: p.id === selectedId }"
            @click="selectedId = p.id"
          >
            <td>{{ p.name }}</td>
            <td>{{ p.vendor }}</td>
            <td class="mono">{{ p.product_code }}</td>
            <td class="mono">{{ p.revision }}</td>
            <td>{{ p.registers.length }}</td>
          </tr>
        </tbody>
      </table>

      <div v-if="selected" class="profile-detail">
        <div class="form-row">
          <label>{{ t("applyProfile") }}</label>
          <select v-model="applyKey">
            <option :value="null">—</option>
            <option v-for="r in devices.records" :key="r.key" :value="r.key">{{ r.key }}</option>
          </select>
          <button class="primary" :disabled="!applyKey" @click="apply">{{ t("applyProfile") }}</button>
        </div>

        <table class="data">
          <thead>
            <tr><th>FC</th><th>{{ t("address") }}</th><th>{{ t("nameCol") }}</th><th>{{ t("type") }}</th><th>{{ t("byteOrder") }}</th><th>{{ t("unitShort") }}</th></tr>
          </thead>
          <tbody>
            <tr v-for="r in selected.registers" :key="`${r.function}:${r.address}`">
              <td>0x{{ r.function.toString(16).padStart(2, "0").toUpperCase() }}</td>
              <td class="mono">0x{{ r.address.toString(16).padStart(4, "0").toUpperCase() }}</td>
              <td>{{ r.name }}</td>
              <td>{{ r.spec.data_type.toUpperCase() }}</td>
              <td>{{ r.spec.byte_order.toUpperCase() }}</td>
              <td>{{ r.unit ?? "—" }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<style scoped>
.form-row {
  flex-wrap: wrap;
}

.profiles-grid {
  margin-top: 10px;
}

.profile-detail {
  margin-top: 12px;
  border-top: 1px solid var(--border);
  padding-top: 8px;
}

tr.selected td {
  background: var(--bg-input);
}

.hint,
.msg {
  color: var(--fg-dim);
  font-size: 12px;
  margin-top: 6px;
}

.error-text {
  color: var(--error);
  font-size: 12px;
}
</style>

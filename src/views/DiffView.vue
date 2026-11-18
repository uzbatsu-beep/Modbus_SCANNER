<script setup lang="ts">
import { t } from "@/i18n";
import { computed, onMounted, ref } from "vue";

import { api } from "@/api/mod";
import type { ChangeKind, ScanDiff, ScanResults } from "@/types/ipc";
import { useSettingsStore } from "@/stores/settings";

interface Source {
  id: string;
  label: string;
  results: ScanResults;
}

const settings = useSettingsStore();
const sources = ref<Source[]>([]);
const leftId = ref("current");
const rightId = ref("");
const diff = ref<ScanDiff | null>(null);
const kindFilter = ref<ChangeKind | "all">("all");
const deviceFilter = ref<string | null>(null);
const error = ref<string | null>(null);

function changeLabel(kind: ChangeKind): string {
  return t(
    kind === "added"
      ? "changeAdded"
      : kind === "removed"
        ? "changeRemoved"
        : kind === "changed"
          ? "changeChanged"
          : "changeUnchanged"
  );
}

const tagClass: Record<ChangeKind, string> = {
  added: "ok",
  removed: "error",
  changed: "warn",
  unchanged: "dim",
};

onMounted(async () => {
  const list: Source[] = [];
  try {
    list.push({ id: "current", label: t("currentSession"), results: await api.scanner.getResults() });
  } catch (e) {
    error.value = String(e);
  }
  const project = settings.loadedProject;
  if (project) {
    list.push({
      id: "project-current",
      label: `${project.name} — ${t("currentResults")}`,
      results: project.results,
    });
    project.history.forEach((snapshot, i) => {
      list.push({
        id: `snapshot-${i}`,
        label: `${project.name} — ${new Date(snapshot.timestamp_ms).toLocaleString()} (${snapshot.label})`,
        results: snapshot.results,
      });
    });
  }
  sources.value = list;
  if (list.length > 1) rightId.value = list[1].id;
});

async function compare() {
  error.value = null;
  const left = sources.value.find((s) => s.id === leftId.value);
  const right = sources.value.find((s) => s.id === rightId.value);
  if (!left || !right) return;
  try {
    diff.value = await api.scanner.compareScans(
      left.results,
      right.results,
      left.label,
      right.label
    );
    deviceFilter.value = null;
  } catch (e) {
    error.value = String(e);
  }
}

const devices = computed(() => diff.value?.devices.map((d) => d.device_key) ?? []);
const rows = computed(() => {
  if (!diff.value) return [];
  return diff.value.registers.filter(
    (r) =>
      (kindFilter.value === "all" || r.change === kindFilter.value) &&
      (!deviceFilter.value || r.device_key === deviceFilter.value)
  );
});

function hex(value: number | null | undefined): string {
  return value == null ? "—" : `0x${value.toString(16).padStart(4, "0")}`;
}
</script>

<template>
  <div class="panel">
    <div class="panel-title">{{ t("diffTitle") }}</div>

    <div class="form-row">
      <label>{{ t("before") }}</label>
      <select v-model="leftId">
        <option v-for="s in sources" :key="s.id" :value="s.id">{{ s.label }}</option>
      </select>
      <label class="short">{{ t("after") }}</label>
      <select v-model="rightId">
        <option v-for="s in sources" :key="s.id" :value="s.id">{{ s.label }}</option>
      </select>
      <button class="primary" :disabled="!leftId || !rightId || leftId === rightId" @click="compare">
        {{ t("compare") }}
      </button>
      <span v-if="error" class="error-text">{{ error }}</span>
    </div>

    <div v-if="!settings.loadedProject" class="hint">
      {{ t("loadProjectHint") }}
    </div>

    <template v-if="diff">
      <div class="summary">
        <span class="tag ok">+ {{ diff.added }}</span>
        <span class="tag warn">~ {{ diff.changed }}</span>
        <span class="tag error">− {{ diff.removed }}</span>
        <span class="dim">
          {{ t("devicesTitle") }}: {{ diff.left_devices }} → {{ diff.right_devices }}
          · {{ t("optionAdded") }} {{ diff.added_devices.length }} · {{ t("optionRemoved") }} {{ diff.removed_devices.length }}
        </span>
      </div>

      <div v-if="diff.added_devices.length || diff.removed_devices.length" class="device-changes">
        <div v-if="diff.added_devices.length" class="mono">
          {{ t("newDevices") }}: {{ diff.added_devices.join(", ") }}
        </div>
        <div v-if="diff.removed_devices.length" class="mono">
          {{ t("goneDevices") }}: {{ diff.removed_devices.join(", ") }}
        </div>
      </div>

      <div class="form-row">
        <label>{{ t("changeType") }}</label>
        <select v-model="kindFilter">
          <option value="all">{{ t("all") }}</option>
          <option value="added">{{ t("optionAdded") }}</option>
          <option value="changed">{{ t("optionChanged") }}</option>
          <option value="removed">{{ t("optionRemoved") }}</option>
        </select>
        <label class="short">{{ t("devicesTitle") }}</label>
        <select v-model="deviceFilter">
          <option :value="null">{{ t("all") }}</option>
          <option v-for="d in devices" :key="d" :value="d">{{ d }}</option>
        </select>
      </div>

      <table class="data">
        <thead>
          <tr><th>{{ t("devicesTitle") }}</th><th>FC</th><th>{{ t("address") }}</th><th>{{ t("beforeValue") }}</th><th>{{ t("afterValue") }}</th><th>{{ t("changeHeader") }}</th></tr>
        </thead>
        <tbody>
          <tr v-for="(r, i) in rows" :key="`${r.device_key}-${r.function}-${r.address}-${i}`">
            <td class="mono">{{ r.device_key }}</td>
            <td>0x{{ r.function.toString(16).padStart(2, "0").toUpperCase() }}</td>
            <td class="mono">0x{{ r.address.toString(16).padStart(4, "0").toUpperCase() }}</td>
            <td class="mono">{{ hex(r.old_value) }}</td>
            <td class="mono">{{ hex(r.new_value) }}</td>
            <td><span class="tag" :class="tagClass[r.change]">{{ changeLabel(r.change) }}</span></td>
          </tr>
        </tbody>
      </table>
      <div v-if="rows.length === 0" class="hint">{{ t("noChanges") }}</div>
    </template>
  </div>
</template>

<style scoped>
.summary {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 10px 0;
}

.device-changes {
  font-size: 12px;
  color: var(--fg-dim);
  margin-bottom: 10px;
}

.hint,
.dim,
.error-text {
  color: var(--fg-dim);
  font-size: 12px;
}

.error-text {
  color: var(--error);
}
</style>

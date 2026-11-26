<script setup lang="ts">
import { t } from "@/i18n";
import { computed, reactive, ref, watch } from "vue";

import { api } from "@/api/mod";
import type {
  ByteOrder,
  DataType,
  DecodeResult,
  DecodedValue,
  DeviceRecord,
  TagDefinition,
  RegisterValue,
  RegisterSpan,
} from "@/types/ipc";
import { useSettingsStore } from "@/stores/settings";
import { addressSpace, readFunction, registerWidth, readSpan, newTagId } from "@/types/registerIdentity";

const props = defineProps<{
  record: DeviceRecord;
  selection: RegisterValue[];
}>();

const settings = useSettingsStore();

const result = ref<DecodeResult | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);
let refreshRevision = 0;
const values = computed(() => props.selection.map(r => r.raw));
const addresses = computed(() => props.selection.map(r => r.address));
const decodedTags = ref<Record<string, string>>({});

const form = reactive({
  name: "",
  data_type: "float32" as DataType,
  byte_order: "abcd" as ByteOrder,
  multiplier: 1,
  divider: 1,
  offset: 0,
  unit: "",
});

const tags = computed<TagDefinition[]>(() =>
  (settings.config.tags ?? []).filter((tag) => tag.source.start.device_key === props.record.key)
);

const allRegisters = computed(() => Object.values(props.record.registers).flatMap(r => r ? [r] : []));
const source = computed<RegisterSpan | null>(() => {
  const first = props.selection[0];
  const space = first && addressSpace(first.function);
  const quantity = registerWidth(form.data_type);
  if (!first || !space || props.selection.length < quantity ||
      (first.function <= 2 && form.data_type !== "uint16")) return null;
  const span = { start: { device_key: props.record.key, address_space: space, address: first.address }, quantity };
  return readSpan(allRegisters.value, span) === null ? null : span;
});
const canSave = computed(() => !!source.value && !!form.name.trim() &&
  Number.isFinite(form.multiplier) && Number.isFinite(form.divider) && form.divider !== 0 && Number.isFinite(form.offset));

const valueNames: Record<DataType, string> = {
  int16: "INT16",
  uint16: "UINT16",
  int32: "INT32",
  uint32: "UINT32",
  float32: "FLOAT32",
  ascii: "ASCII",
};

function formatValue(value: DecodedValue): string {
  if (value.kind === "float") {
    return Number.isInteger(value.value) ? value.value.toFixed(2) : String(value.value);
  }
  return String(value.value);
}

async function refresh() {
  const revision = ++refreshRevision;
  const selected = [...values.value];
  const snapshot = [...allRegisters.value];
  const named = [...tags.value];
  loading.value = true;
  error.value = null;
  result.value = null;
  decodedTags.value = {};
  try {
    const inspected = selected.length ? await api.modbus.inspectRegisters(selected) : null;
    const decoded: Record<string, string> = {};
    for (const tag of named) {
      const raws = readSpan(snapshot, tag.source);
      if (raws === null) continue;
      const value = await api.modbus.decodeRegister(raws, tag.spec);
      if (value) decoded[tag.id] = formatValue(value);
    }
    if (revision === refreshRevision) {
      result.value = inspected;
      decodedTags.value = decoded;
    }
  } catch (e) {
    if (revision === refreshRevision) error.value = String(e);
  } finally {
    if (revision === refreshRevision) loading.value = false;
  }
}

watch(() => [props.record.key, props.selection, allRegisters.value, tags.value],
  () => void refresh(), { immediate: true, deep: true });

async function saveTag() {
  if (!canSave.value || !source.value) return;
  const tag: TagDefinition = {
    id: newTagId(),
    source: source.value,
    name: form.name.trim(),
    spec: {
      data_type: form.data_type,
      byte_order: form.byte_order,
      multiplier: Number(form.multiplier),
      divider: Number(form.divider),
      offset: Number(form.offset),
    },
    unit: form.unit.trim() || null,
    description: null,
    profile_id: null,
  };
  const previous = settings.config.tags;
  settings.config.tags = [...previous, tag];
  try {
    await settings.save();
    form.name = "";
  } catch (e) {
    settings.config.tags = previous;
    error.value = String(e);
  }
}
async function removeTag(id: string) {
  const previous = settings.config.tags;
  settings.config.tags = previous.filter(tag => tag.id !== id);
  try { await settings.save(); }
  catch (e) { settings.config.tags = previous; error.value = String(e); }
}
</script>

<template>
  <div class="section inspector">
    <div class="section-title">{{ t("inspectorTitle") }}</div>

    <div v-if="values.length === 0" class="hint">
      {{ t("selectRegisters") }}
    </div>

    <template v-else>
      <div class="raw-addresses mono">
        {{ t("inspectorAddresses") }}: FC{{ selection[0]?.function.toString().padStart(2, "0") }} · {{ addresses.map((a) => "0x" + a.toString(16).padStart(4, "0")).join(" · ") }}
        · raw: {{ values.map((v) => "0x" + v.toString(16).padStart(4, "0")).join(" ") }}
      </div>

      <div v-if="loading" class="hint">{{ t("decoding") }}</div>

      <table v-else-if="result" class="data">
        <thead>
          <tr><th>{{ t("type") }}</th><th>{{ t("byteOrder") }}</th><th>{{ t("value") }}</th></tr>
        </thead>
        <tbody>
          <tr v-for="(item, i) in result.interpretations" :key="i">
            <td>{{ valueNames[item.data_type] }}</td>
            <td class="dim">{{ item.byte_order.toUpperCase() }}</td>
            <td class="mono">{{ formatValue(item.value) }}</td>
          </tr>
        </tbody>
      </table>

      <div v-if="result?.suggestions.length" class="suggestions">
        <span class="tag warn">{{ t("hypotheses") }}</span>
        <span v-for="(s, i) in result.suggestions" :key="i" class="suggestion mono">
          {{ valueNames[s.data_type] }} ({{ s.byte_order.toUpperCase() }}) ≈ {{ s.value.toFixed(3) }}
        </span>
      </div>

      <div class="tag-form">
        <div class="form-row">
          <label>{{ t("tagName") }}</label>
          <input v-model="form.name" placeholder="Voltage_L1" />
          <label class="short">{{ t("type") }}</label>
          <select v-model="form.data_type">
            <option value="int16">INT16</option>
            <option value="uint16">UINT16</option>
            <option value="int32">INT32</option>
            <option value="uint32">UINT32</option>
            <option value="float32">FLOAT32</option>
            <option value="ascii">ASCII</option>
          </select>
          <label class="short">{{ t("byteOrder") }}</label>
          <select v-model="form.byte_order" >
            <option value="abcd">ABCD</option>
            <option value="badc">BADC</option>
            <option value="cdab">CDAB</option>
            <option value="dcba">DCBA</option>
          </select>
        </div>
        <div class="form-row">
          <label>{{ t("scale") }}</label>
          <input v-model.number="form.multiplier" type="number" step="any" class="num" />
          <input v-model.number="form.divider" type="number" step="any" class="num" />
          <input v-model.number="form.offset" type="number" step="any" class="num" />
          <label class="short">{{ t("unitShort") }}</label>
          <input v-model="form.unit" placeholder="V" class="num" />
          <button class="primary" :disabled="!canSave || settings.saving" @click="saveTag">{{ t("saveTag") }}</button>
        </div>
      </div>
    </template>

    <div v-if="error" class="hint">{{ error }}</div>
    <div v-if="tags.length" class="saved-tags">
      <div class="section-title">{{ t("namedTags") }}</div>
      <table class="data">
        <thead>
          <tr><th>{{ t("nameCol") }}</th><th>{{ t("address") }}</th><th>{{ t("type") }}</th><th>{{ t("value") }}</th><th /></tr>
        </thead>
        <tbody>
          <tr v-for="tag in tags" :key="tag.id">
            <td>{{ tag.name }}</td>
            <td class="mono">FC{{ readFunction(tag.source.start.address_space).toString().padStart(2, "0") }} · 0x{{ tag.source.start.address.toString(16).padStart(4, "0") }} × {{ tag.source.quantity }}</td>
            <td class="dim">{{ tag.spec.data_type.toUpperCase() }} / {{ tag.spec.byte_order.toUpperCase() }}</td>
            <td class="mono">
              {{ decodedTags[tag.id] ?? "—" }}
              <span v-if="tag.unit" class="dim">{{ tag.unit }}</span>
            </td>
            <td><button :disabled="settings.saving" @click="removeTag(tag.id)">×</button></td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.inspector {
  margin-top: 12px;
  border-top: 1px solid var(--border);
  padding-top: 10px;
}

.raw-addresses {
  font-size: 12px;
  color: var(--fg-dim);
  margin-bottom: 6px;
}

.suggestions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin: 8px 0;
}

.suggestion {
  font-size: 11px;
  color: var(--warn);
}

.tag-form {
  margin-top: 8px;
  padding: 8px;
  background: var(--bg-input);
  border-radius: 6px;
}

.num {
  width: 90px;
}

.short {
  min-width: auto !important;
}

.dim {
  color: var(--fg-dim);
}

.hint {
  font-size: 11px;
  color: var(--fg-dim);
}

.saved-tags {
  margin-top: 10px;
}
</style>

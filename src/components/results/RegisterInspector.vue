<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";

import { api } from "@/api/mod";
import type {
  ByteOrder,
  DataType,
  DecodeResult,
  DeviceRecord,
  TagDefinition,
} from "@/types/ipc";
import { useSettingsStore } from "@/stores/settings";

const props = defineProps<{
  record: DeviceRecord;
  values: number[];
  addresses: number[];
}>();

const settings = useSettingsStore();

const result = ref<DecodeResult | null>(null);
const loading = ref(false);
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
  (settings.config.tags ?? []).filter((t) => t.device_key === props.record.key)
);

const baseAddress = computed(() => props.addresses[0] ?? 0);
const needsTwo = computed(() =>
  ["int32", "uint32", "float32"].includes(form.data_type)
);

const valueNames: Record<DataType, string> = {
  int16: "INT16",
  uint16: "UINT16",
  int32: "INT32",
  uint32: "UINT32",
  float32: "FLOAT32",
  ascii: "ASCII",
};

function formatValue(value: { kind: string; value: number | string }): string {
  if (value.kind === "float") {
    const v = Number(value.value);
    return Number.isInteger(v) ? v.toFixed(2) : String(v);
  }
  return String(value.value);
}

async function refresh() {
  decodedTags.value = {};
  if (props.values.length === 0) {
    result.value = null;
    return;
  }
  loading.value = true;
  try {
    result.value = await api.modbus.inspectRegisters(props.values);
  } finally {
    loading.value = false;
  }
  for (const tag of tags.value) {
    const width = ["int32", "uint32", "float32"].includes(tag.spec.data_type) ? 2 : 1;
    const raws: number[] = [];
    for (let i = 0; i < width; i++) {
      const reg = Object.values(props.record.registers).find(
        (r) => r.address === tag.address + i
      );
      raws.push(reg?.raw ?? 0);
    }
    if (raws.some((v) => v === 0) && width === 2) continue;
    const decoded = await api.modbus.decodeRegister(raws, tag.spec);
    if (decoded) decodedTags.value[`${tag.address}`] = formatValue(decoded);
  }
}

watch(
  () => [props.values.join(","), tags.value.length],
  () => void refresh(),
  { immediate: true }
);

async function saveTag() {
  if (!form.name.trim()) return;
  const tag: TagDefinition = {
    device_key: props.record.key,
    address: baseAddress.value,
    name: form.name.trim(),
    spec: {
      data_type: form.data_type,
      byte_order: form.byte_order,
      multiplier: Number(form.multiplier),
      divider: Number(form.divider) || 1,
      offset: Number(form.offset),
    },
    unit: form.unit.trim() || undefined,
    description: undefined,
  };
  settings.config.tags = [
    ...(settings.config.tags ?? []).filter(
      (t) => !(t.device_key === tag.device_key && t.address === tag.address)
    ),
    tag,
  ];
  await settings.save();
  form.name = "";
  await refresh();
}

async function removeTag(address: number) {
  settings.config.tags = (settings.config.tags ?? []).filter(
    (t) => !(t.device_key === props.record.key && t.address === address)
  );
  await settings.save();
  await refresh();
}
</script>

<template>
  <div class="section inspector">
    <div class="section-title">Интерпретация регистров / Register inspector</div>

    <div v-if="values.length === 0" class="hint">
      Выберите 1–2 регистра в таблице выше (для 32-битных значений нужны два).
    </div>

    <template v-else>
      <div class="raw-addresses mono">
        Адреса: {{ addresses.map((a) => "0x" + a.toString(16).padStart(4, "0")).join(" · ") }}
        · raw: {{ values.map((v) => "0x" + v.toString(16).padStart(4, "0")).join(" ") }}
      </div>

      <div v-if="loading" class="hint">декодирование…</div>

      <table v-else-if="result" class="data">
        <thead>
          <tr><th>Тип</th><th>Byte order</th><th>Значение</th></tr>
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
        <span class="tag warn">возможные варианты (гипотезы)</span>
        <span v-for="(s, i) in result.suggestions" :key="i" class="suggestion mono">
          {{ valueNames[s.data_type] }} ({{ s.byte_order.toUpperCase() }}) ≈ {{ s.value.toFixed(3) }}
        </span>
      </div>

      <div class="tag-form">
        <div class="form-row">
          <label>Имя тега</label>
          <input v-model="form.name" placeholder="Voltage_L1" />
          <label class="short">Тип</label>
          <select v-model="form.data_type">
            <option value="int16">INT16</option>
            <option value="uint16">UINT16</option>
            <option value="int32">INT32</option>
            <option value="uint32">UINT32</option>
            <option value="float32">FLOAT32</option>
            <option value="ascii">ASCII</option>
          </select>
          <label class="short">Byte order</label>
          <select v-model="form.byte_order" :disabled="!needsTwo && form.data_type !== 'ascii'">
            <option value="abcd">ABCD</option>
            <option value="badc">BADC</option>
            <option value="cdab">CDAB</option>
            <option value="dcba">DCBA</option>
          </select>
        </div>
        <div class="form-row">
          <label>Масштаб (× / ÷ / +)</label>
          <input v-model.number="form.multiplier" type="number" step="any" class="num" />
          <input v-model.number="form.divider" type="number" step="any" class="num" />
          <input v-model.number="form.offset" type="number" step="any" class="num" />
          <label class="short">Единица</label>
          <input v-model="form.unit" placeholder="V" class="num" />
          <button class="primary" :disabled="!form.name.trim()" @click="saveTag">Сохранить тег</button>
        </div>
      </div>
    </template>

    <div v-if="tags.length" class="saved-tags">
      <div class="section-title">Именованные теги устройства</div>
      <table class="data">
        <thead>
          <tr><th>Имя</th><th>Адрес</th><th>Тип</th><th>Значение</th><th /></tr>
        </thead>
        <tbody>
          <tr v-for="tag in tags" :key="`${tag.address}-${tag.name}`">
            <td>{{ tag.name }}</td>
            <td class="mono">0x{{ tag.address.toString(16).padStart(4, "0") }}</td>
            <td class="dim">{{ tag.spec.data_type.toUpperCase() }} / {{ tag.spec.byte_order.toUpperCase() }}</td>
            <td class="mono">
              {{ decodedTags[`${tag.address}`] ?? "—" }}
              <span v-if="tag.unit" class="dim">{{ tag.unit }}</span>
            </td>
            <td><button @click="removeTag(tag.address)">×</button></td>
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

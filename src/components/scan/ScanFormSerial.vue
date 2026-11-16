<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";

import { api } from "@/api/mod";
import type { ScanParams, ScanTarget } from "@/types/ipc";
import { BAUD_RATES } from "@/types/ipc";
import { useScannerStore } from "@/stores/scanner";
import { useSerialStore } from "@/stores/serial";
import { useSettingsStore } from "@/stores/settings";

const props = defineProps<{ params: ScanParams }>();
const emit = defineEmits<{ (e: "start", target: ScanTarget): void }>();

const scanner = useScannerStore();
const serial = useSerialStore();
const settings = useSettingsStore();

const form = reactive({
  port_name: settings.config.last_serial?.port_name ?? "",
  baud_rate: settings.config.last_serial?.baud_rate ?? 9600,
  data_bits: (settings.config.last_serial?.data_bits ?? "eight") as "five" | "six" | "seven" | "eight",
  parity: (settings.config.last_serial?.parity ?? "none") as "none" | "odd" | "even",
  stop_bits: (settings.config.last_serial?.stop_bits ?? "one") as "one" | "two",
  mode: "rtu" as "rtu" | "ascii",
  unit_from: 1,
  unit_to: 247,
});

const testing = ref(false);
const testResult = ref<string | null>(null);

const canStart = computed(
  () =>
    form.port_name.length > 0 &&
    form.unit_from >= 0 &&
    form.unit_to <= 247 &&
    form.unit_from <= form.unit_to &&
    scanner.canStart
);

onMounted(() => {
  serial.refresh().then(() => {
    if (!form.port_name && serial.ports.length > 0) {
      form.port_name = serial.ports[0].port_name;
    }
  });
});

async function testPort() {
  testing.value = true;
  testResult.value = null;
  try {
    const res = await api.serial.testPort({
      port_name: form.port_name,
      baud_rate: form.baud_rate,
      data_bits: Number(form.data_bits === "eight" ? 8 : form.data_bits === "seven" ? 7 : form.data_bits === "six" ? 6 : 5),
      parity: form.parity,
      stop_bits: Number(form.stop_bits === "two" ? 2 : 1),
      mode: form.mode,
      test_modbus: true,
    });
    if (res.error && !res.modbus_responded) {
      testResult.value = `ошибка: ${res.error}`;
    } else if (res.modbus_responded) {
      testResult.value = `порт открыт, Modbus ответил (${res.response_hex})`;
    } else {
      testResult.value = "порт открыт, нет ответа Modbus";
    }
  } catch (e) {
    testResult.value = `ошибка: ${e}`;
  } finally {
    testing.value = false;
  }
}

function start() {
  const target: ScanTarget = {
    type: "unit_id_range",
    from: form.unit_from,
    to: form.unit_to,
    port: {
      port_name: form.port_name,
      baud_rate: form.baud_rate,
      data_bits: form.data_bits,
      parity: form.parity,
      stop_bits: form.stop_bits,
    },
    mode: form.mode,
  };
  emit("start", target);
}
</script>

<template>
  <div class="panel">
    <div class="panel-title">Modbus RTU / ASCII — сканер шины (unit id)</div>

    <div class="form-row">
      <label>Порт / Port</label>
      <select v-model="form.port_name" :disabled="scanner.isRunning">
        <option v-for="p in serial.ports" :key="p.port_name" :value="p.port_name">
          {{ p.port_name }} ({{ p.port_type }}{{ p.product ? " · " + p.product : "" }})
        </option>
        <option v-if="serial.ports.length === 0" value="">— нет портов —</option>
      </select>
      <button :disabled="testing || scanner.isRunning" @click="serial.refresh()">↻</button>
      <button :disabled="testing || scanner.isRunning" @click="testPort">Тест</button>
      <span v-if="testResult" class="test-result">{{ testResult }}</span>
    </div>

    <div class="form-row">
      <label>Режим / Mode</label>
      <select v-model="form.mode" :disabled="scanner.isRunning">
        <option value="rtu">RTU (binary + CRC-16)</option>
        <option value="ascii">ASCII (hex + LRC)</option>
      </select>
    </div>

    <div class="form-row">
      <label>Скорость / Baud</label>
      <select v-model.number="form.baud_rate" :disabled="scanner.isRunning">
        <option v-for="b in BAUD_RATES" :key="b" :value="b">{{ b }}</option>
      </select>
      <label class="short">Биты</label>
      <select v-model="form.data_bits" :disabled="scanner.isRunning">
        <option value="eight">8</option>
        <option value="seven">7</option>
      </select>
      <label class="short">Чётность</label>
      <select v-model="form.parity" :disabled="scanner.isRunning">
        <option value="none">N</option>
        <option value="even">E</option>
        <option value="odd">O</option>
      </select>
      <label class="short">Стоп</label>
      <select v-model="form.stop_bits" :disabled="scanner.isRunning">
        <option value="one">1</option>
        <option value="two">2</option>
      </select>
    </div>

    <div class="form-row">
      <label>Unit ID: от — до</label>
      <input v-model.number="form.unit_from" type="number" min="0" max="247" :disabled="scanner.isRunning" />
      <span>—</span>
      <input v-model.number="form.unit_to" type="number" min="0" max="247" :disabled="scanner.isRunning" />
      <span class="hint">0 = broadcast (ответа нет), 248–255 зарезервированы</span>
    </div>

    <div class="form-row">
      <label />
      <button class="primary" :disabled="!canStart" @click="start">
        ▶ Сканировать шину ({{ form.unit_to - form.unit_from + 1 }} адресов)
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

.test-result {
  font-family: var(--mono);
  font-size: 11px;
  color: var(--fg-dim);
}
</style>

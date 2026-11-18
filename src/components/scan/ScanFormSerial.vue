<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";

import { api } from "@/api/mod";
import { t } from "@/i18n";
import type {
  ParitySerde,
  ScanParams,
  ScanTarget,
  SerialFraming,
} from "@/types/ipc";
import { BAUD_RATES } from "@/types/ipc";
import { useScannerStore } from "@/stores/scanner";
import { useSerialStore } from "@/stores/serial";
import { useSettingsStore } from "@/stores/settings";

defineProps<{ params: ScanParams }>();
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

// P2-01: automatic serial-settings sweep.
const autoSweep = ref(false);
const selectedBauds = ref<number[]>([9600, 2400, 19200, 4800, 38400, 57600, 115200, 1200]);
const selectedParities = ref<ParitySerde[]>(["none", "even", "odd"]);
const parityOptions: ParitySerde[] = ["none", "even", "odd"];

function toggleBaud(baud: number) {
  const i = selectedBauds.value.indexOf(baud);
  if (i >= 0) selectedBauds.value.splice(i, 1);
  else selectedBauds.value.push(baud);
}

function toggleParity(parity: ParitySerde) {
  const i = selectedParities.value.indexOf(parity);
  if (i >= 0) selectedParities.value.splice(i, 1);
  else selectedParities.value.push(parity);
}

const canStart = computed(
  () =>
    form.port_name.length > 0 &&
    form.unit_from >= 0 &&
    form.unit_to <= 247 &&
    form.unit_from <= form.unit_to &&
    (!autoSweep.value ||
      (selectedBauds.value.length > 0 && selectedParities.value.length > 0)) &&
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
  if (autoSweep.value) {
    const framings: SerialFraming[] = selectedParities.value.map((parity) => ({
      data_bits: "eight",
      parity,
      stop_bits: "one",
    }));
    const target: ScanTarget = {
      type: "serial_auto",
      sweep: {
        port_name: form.port_name,
        mode: form.mode,
        baud_rates: [...selectedBauds.value],
        framings,
        discovery_unit_ids: [1, 2, 3, 4, 5, 10, 20, 100, 247],
        unit_from: form.unit_from,
        unit_to: form.unit_to,
      },
    };
    emit("start", target);
    return;
  }

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
    <div class="panel-title">{{ t("scanPanelTitle") }}</div>

    <div class="form-row">
      <label>{{ t("port") }}</label>
      <select v-model="form.port_name" :disabled="scanner.isRunning">
        <option v-for="p in serial.ports" :key="p.port_name" :value="p.port_name">
          {{ p.port_name }} ({{ p.port_type }}{{ p.product ? " · " + p.product : "" }})
        </option>
        <option v-if="serial.ports.length === 0" value="">{{ t("noPorts") }}</option>
      </select>
      <button :disabled="testing || scanner.isRunning" @click="serial.refresh()">↻</button>
      <button :disabled="testing || scanner.isRunning" @click="testPort">{{ t("test") }}</button>
      <span v-if="testResult" class="test-result">{{ testResult }}</span>
    </div>

    <div class="form-row">
      <label>{{ t("mode") }}</label>
      <select v-model="form.mode" :disabled="scanner.isRunning">
        <option value="rtu">RTU (binary + CRC-16)</option>
        <option value="ascii">ASCII (hex + LRC)</option>
      </select>
    </div>

    <div class="form-row">
      <label>{{ t("autoSettings") }}</label>
      <label class="check">
        <input type="checkbox" v-model="autoSweep" :disabled="scanner.isRunning" />
        {{ t("autoSettingsHint") }}
      </label>
      <span v-if="autoSweep" class="hint">
        {{ t("autoDiscoveryHint1") }}
        {{ t("autoDiscoveryHint2") }}
      </span>
    </div>

    <div v-if="autoSweep" class="form-row sweep-options">
      <label>Baud</label>
      <label v-for="b in BAUD_RATES" :key="b" class="check">
        <input
          type="checkbox"
          :checked="selectedBauds.includes(b)"
          :disabled="scanner.isRunning"
          @change="toggleBaud(b)"
        />
        {{ b }}
      </label>
    </div>

    <div v-if="autoSweep" class="form-row sweep-options">
      <label>{{ t("parity") }}</label>
      <label v-for="p in parityOptions" :key="p" class="check">
        <input
          type="checkbox"
          :checked="selectedParities.includes(p)"
          :disabled="scanner.isRunning"
          @change="toggleParity(p)"
        />
        {{ p === "none" ? "N (8N1)" : p === "even" ? "E (8E1)" : "O (8O1)" }}
      </label>
      <span class="hint">{{ t("recommendedTimeout") }}</span>
    </div>

    <div class="form-row" :class="{ dimmed: autoSweep }">
      <label>{{ t("baud") }}</label>
      <select v-model.number="form.baud_rate" :disabled="scanner.isRunning || autoSweep">
        <option v-for="b in BAUD_RATES" :key="b" :value="b">{{ b }}</option>
      </select>
      <label class="short">{{ t("dataBits") }}</label>
      <select v-model="form.data_bits" :disabled="scanner.isRunning || autoSweep">
        <option value="eight">8</option>
        <option value="seven">7</option>
      </select>
      <label class="short">{{ t("parity") }}</label>
      <select v-model="form.parity" :disabled="scanner.isRunning || autoSweep">
        <option value="none">N</option>
        <option value="even">E</option>
        <option value="odd">O</option>
      </select>
      <label class="short">{{ t("stopBits") }}</label>
      <select v-model="form.stop_bits" :disabled="scanner.isRunning || autoSweep">
        <option value="one">1</option>
        <option value="two">2</option>
      </select>
    </div>

    <div class="form-row">
      <label>{{ t("unitRange") }}</label>
      <input v-model.number="form.unit_from" type="number" min="0" max="247" :disabled="scanner.isRunning" />
      <span>—</span>
      <input v-model.number="form.unit_to" type="number" min="0" max="247" :disabled="scanner.isRunning" />
      <span class="hint">{{ t("broadcastHint") }}</span>
    </div>

    <div class="form-row">
      <label />
      <button class="primary" :disabled="!canStart" @click="start">
        <template v-if="autoSweep">
          ▶ {{ t("sweepAndScan") }} {{ form.unit_to - form.unit_from + 1 }} {{ t("addresses") }}
        </template>
        <template v-else>
          ▶ {{ t("scanBus") }} ({{ form.unit_to - form.unit_from + 1 }} {{ t("addresses") }})
        </template>
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

.check {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: auto !important;
  font-size: 12px;
  cursor: pointer;
}

.dimmed {
  opacity: 0.5;
}

.sweep-options {
  flex-wrap: wrap;
}
</style>

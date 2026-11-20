<script setup lang="ts">
import { t } from "@/i18n";
import { computed, onMounted, reactive, ref } from "vue";

import { api } from "@/api/mod";
import type { ManualResponse } from "@/types/ipc";
import { FC, FC_NAMES } from "@/types/ipc";
import { useSettingsStore } from "@/stores/settings";

const settings = useSettingsStore();

const form = reactive({
  kind: "tcp" as "rtu" | "ascii" | "tcp" | "udp",
  endpoint: "192.168.1.1",
  port: 502,
  unit_id: 1,
  // serial
  port_name: "COM3",
  baud_rate: 9600,
  // request
  read_fc: FC.READ_HOLDING_REGISTERS as number,
  read_addr: 0,
  read_qty: 10,
  write_fc: FC.WRITE_SINGLE_REGISTER as number,
  write_addr: 0,
  write_vals: "0" as string,
  raw_hex: "03 00 00 00 0A",
  raw_full_adu: false,
  // timing
  timeout_ms: 1000,
  retries: 0,
});

const busy = ref(false);
const lastResponse = ref<ManualResponse | null>(null);
const lastAction = ref<string>("");

const isSerial = computed(() => form.kind === "rtu" || form.kind === "ascii");
const serialMode = computed<"rtu" | "ascii">((): "rtu" | "ascii" =>
  form.kind === "ascii" ? "ascii" : "rtu"
);

function buildTarget() {
  return {
    kind: form.kind,
    endpoint: isSerial.value ? form.port_name : form.endpoint,
    port: isSerial.value ? null : form.port,
    unit_id: form.unit_id,
    serial: isSerial.value
      ? {
          port_name: form.port_name,
          baud_rate: form.baud_rate,
          data_bits: "eight" as const,
          parity: "none" as const,
          stop_bits: "one" as const,
        }
      : null,
    serial_mode: isSerial.value ? serialMode.value : null,
  };
}

function buildTiming(retries = form.retries) {
  return { timeout_ms: form.timeout_ms, retries };
}

async function doRead() {
  busy.value = true;
  lastAction.value = `read fc=${form.read_fc.toString(16)} addr=${form.read_addr} qty=${form.read_qty}`;
  try {
    lastResponse.value = await api.modbus.read({
      target: buildTarget(),
      function: form.read_fc,
      start_address: form.read_addr,
      quantity: form.read_qty,
      timing: buildTiming(),
    });
  } catch (e) {
    lastResponse.value = { ok: false, error: String(e), values: [], exception_code: null, response_hex: null, rtt_us: null };
  } finally {
    busy.value = false;
  }
}

async function doWrite() {
  const values = form.write_vals
    .split(/[\s,;]+/)
    .filter((s) => s.length > 0)
    .map((s) => Number.parseInt(s, 16));
  if (values.length === 0 || values.some((v) => !Number.isInteger(v) || v < 0 || v > 0xffff)) {
    window.alert(t("invalidWriteValues"));
    return;
  }
  if (!window.confirm(t("confirmWrite"))) return;
  busy.value = true;
  lastAction.value = `write fc=${form.write_fc.toString(16)} addr=${form.write_addr}`;
  try {
    lastResponse.value = await api.modbus.write({
      target: buildTarget(),
      function: form.write_fc,
      start_address: form.write_addr,
      values,
      confirm_write: true,
      timing: buildTiming(0),
    });
  } catch (e) {
    lastResponse.value = { ok: false, error: String(e), values: [], exception_code: null, response_hex: null, rtt_us: null };
  } finally {
    busy.value = false;
  }
}

async function doRaw() {
  if (!window.confirm(t("confirmUnsafe"))) return;
  busy.value = true;
  lastAction.value = `raw "${form.raw_hex}"`;
  try {
    lastResponse.value = await api.modbus.sendRaw({
      target: buildTarget(),
      adu_hex: form.raw_hex,
      is_full_adu: form.raw_full_adu,
      confirm_unsafe: true,
      timing: buildTiming(0),
    });
  } catch (e) {
    lastResponse.value = { ok: false, error: String(e), values: [], exception_code: null, response_hex: null, rtt_us: null };
  } finally {
    busy.value = false;
  }
}

onMounted(() => {
  form.endpoint = settings.config.last_tcp_host ?? form.endpoint;
  form.port = settings.config.last_tcp_port ?? form.port;
});
</script>

<template>
  <div class="manual-view">
    <div class="panel">
      <div class="panel-title">{{ t("manualTitle") }}</div>

      <div class="form-row">
        <label>{{ t("transport") }}</label>
        <select v-model="form.kind">
          <option value="tcp">TCP</option>
          <option value="udp">UDP</option>
          <option value="rtu">RTU (serial)</option>
          <option value="ascii">ASCII (serial)</option>
        </select>
        <label class="short">Unit ID</label>
        <input v-model.number="form.unit_id" type="number" min="0" max="255" />
      </div>

      <div v-if="!isSerial" class="form-row">
        <label>Хост / Host</label>
        <input v-model="form.endpoint" placeholder="192.168.1.1" />
        <label class="short">Порт</label>
        <input v-model.number="form.port" type="number" min="1" max="65535" />
      </div>

      <div v-else class="form-row">
        <label>{{ t("port") }}</label>
        <input v-model="form.port_name" placeholder="COM3 / /dev/ttyUSB0" />
        <label class="short">Baud</label>
        <input v-model.number="form.baud_rate" type="number" />
      </div>

      <div class="form-row">
        <label>{{ t("timeout") }}</label>
        <input v-model.number="form.timeout_ms" type="number" min="50" max="10000" />
        <label class="short">{{ t("retries") }}</label>
        <input v-model.number="form.retries" type="number" min="0" max="5" />
      </div>
      <div class="hint">{{ t("manualReadRetryHint") }}</div>
    </div>

    <!-- READ -->
    <div class="panel">
      <div class="panel-title">{{ t("read") }}</div>
      <div class="form-row">
        <label>{{ t("function") }}</label>
        <select v-model.number="form.read_fc">
          <option :value="0x01">{{ FC_NAMES[0x01] }}</option>
          <option :value="0x02">{{ FC_NAMES[0x02] }}</option>
          <option :value="0x03">{{ FC_NAMES[0x03] }}</option>
          <option :value="0x04">{{ FC_NAMES[0x04] }}</option>
        </select>
        <label class="short">{{ t("addressHex") }}</label>
        <input v-model.number="form.read_addr" type="number" min="0" max="65535" />
        <label class="short">{{ t("quantity") }}</label>
        <input v-model.number="form.read_qty" type="number" min="1" max="125" />
        <button class="primary" :disabled="busy" @click="doRead">▶ {{ t("readBtn") }}</button>
      </div>
    </div>

    <!-- WRITE -->
    <div class="panel">
      <div class="panel-title">{{ t("write") }}</div>
      <div class="form-row">
        <label>{{ t("function") }}</label>
        <select v-model.number="form.write_fc">
          <option :value="0x05">{{ FC_NAMES[0x05] }}</option>
          <option :value="0x06">{{ FC_NAMES[0x06] }}</option>
          <option :value="0x10">{{ FC_NAMES[0x10] }}</option>
        </select>
        <label class="short">Адрес</label>
        <input v-model.number="form.write_addr" type="number" min="0" max="65535" />
      </div>
      <div class="hint">{{ t("writeSafetyHint") }}</div>
      <div class="form-row">
        <label>{{ t("valuesHex") }}</label>
        <input v-model="form.write_vals" placeholder="0x1234, 0xABCD или 1234 ABCD" class="wide" />
        <button class="warn" :disabled="busy" @click="doWrite">✎ {{ t("writeBtn") }}</button>
      </div>
    </div>

    <!-- RAW -->
    <div class="panel">
      <div class="panel-title">{{ t("rawFrame") }}</div>
      <div class="form-row">
        <label>ADU hex</label>
        <input v-model="form.raw_hex" placeholder="03 00 00 00 0A" class="wide mono" />
        <label class="check">
          <input type="checkbox" v-model="form.raw_full_adu" />
          {{ t("fullAdu") }}
        </label>
        <button :disabled="busy" @click="doRaw()">▶ {{ t("send") }}</button>
      </div>
      <div class="hint">
        {{ t("rawHint") }}
      </div>
      <div class="hint">{{ t("rawResponseCaution") }}</div>
    </div>

    <!-- RESPONSE -->
    <div class="panel" v-if="lastResponse">
      <div class="panel-title">{{ t("response") }} ({{ lastAction }})</div>

      <div class="form-row">
        <label>{{ t("status") }}</label>
        <span class="tag" :class="lastResponse.ok ? 'ok' : 'error'">
          {{ lastResponse.ok ? "OK" : lastResponse.error ?? "ERROR" }}
        </span>
        <template v-if="lastResponse.exception_code != null">
          <span class="tag warn">EXC {{ lastResponse.exception_code.toString(16).toUpperCase() }}</span>
        </template>
        <label v-if="lastResponse.rtt_us != null" class="short">RTT</label>
        <span v-if="lastResponse.rtt_us != null" class="mono">
          {{ (lastResponse.rtt_us / 1000).toFixed(1) }} ms
        </span>
      </div>

      <div v-if="lastResponse.values.length > 0" class="form-row">
        <label>{{ t("values") }}</label>
        <div class="hex vals">
          <span
            v-for="(v, i) in lastResponse.values"
            :key="i"
            class="val"
            :title="String(v)"
          >
            {{ v.toString(16).toUpperCase().padStart(4, "0") }}
          </span>
        </div>
      </div>

      <div v-if="lastResponse.response_hex" class="form-row">
        <label>Raw</label>
        <div class="hex">{{ lastResponse.response_hex }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.short {
  min-width: auto !important;
}

.wide {
  min-width: 320px;
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

.vals .val {
  display: inline-block;
  margin-right: 8px;
  padding: 1px 5px;
  background: var(--bg-input);
  border-radius: 3px;
  border: 1px solid var(--border);
}
</style>

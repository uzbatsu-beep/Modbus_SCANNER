<script setup lang="ts">
import { t } from "@/i18n";
import { computed } from "vue";

import type { ScanParams } from "@/types/ipc";
import { FC, FC_NAMES } from "@/types/ipc";
import { useScannerStore } from "@/stores/scanner";

const params = defineModel<ScanParams>({ required: true });

const scanner = useScannerStore();

function splitEntries(value: string): string[] {
  return value.split(/[\s,;]+/).map((s) => s.trim()).filter(Boolean);
}
const allowlistText = computed({
  get: () => (params.value.ip_allowlist ?? []).join(", "),
  set: (v: string) => { params.value.ip_allowlist = splitEntries(v); },
});
const exclusionsText = computed({
  get: () => (params.value.ip_exclusions ?? []).join(", "),
  set: (v: string) => { params.value.ip_exclusions = splitEntries(v); },
});

const READ_FCS = [FC.READ_COILS, FC.READ_DISCRETE_INPUTS, FC.READ_HOLDING_REGISTERS, FC.READ_INPUT_REGISTERS];

const tcpFraming = computed({
  get: (): "mbap" | "rtu" => params.value.tcp_framing ?? "mbap",
  set: (v: "mbap" | "rtu") => {
    params.value.tcp_framing = v;
  },
});

const tlsEnabled = computed({
  get: () => params.value.tls != null,
  set: (enabled: boolean) => {
    params.value.tls = enabled ? { client_cert: null, client_key: null } : null;
  },
});

function toggleProbe(fc: number) {
  const i = params.value.probe_functions.indexOf(fc);
  if (i >= 0) params.value.probe_functions.splice(i, 1);
  else params.value.probe_functions.push(fc);
}
</script>

<template>
  <div class="panel">
    <div class="panel-title">{{ t("scanParamsTitle") }}</div>

    <div class="form-row">
      <label>{{ t("timeout") }}</label>
      <input v-model.number="params.timeout_ms" type="number" min="50" max="10000" :disabled="scanner.isRunning" />
      <label class="short">{{ t("gap") }}</label>
      <input v-model.number="params.request_gap_ms" type="number" min="0" max="1000" :disabled="scanner.isRunning" />
    </div>

    <div class="form-row">
      <label>{{ t("retries") }}</label>
      <input v-model.number="params.retries" type="number" min="0" max="5" :disabled="scanner.isRunning" />
      <label class="short">{{ t("ipConcurrency") }}</label>
      <input v-model.number="params.ip_concurrency" type="number" min="1" max="256" :disabled="scanner.isRunning" />
    </div>

    <div class="form-row">
      <label>{{ t("otQuietMode") }}</label>
      <input type="checkbox" v-model="params.quiet_mode" :disabled="scanner.isRunning" />
      <span class="hint">{{ t("otQuietHint") }}</span>
    </div>

    <div class="form-row">
      <label>{{ t("otAllowlist") }}</label>
      <input v-model="allowlistText" class="wide" placeholder="192.168.1.0/24, 10.0.0.5" :disabled="scanner.isRunning" />
    </div>
    <div class="form-row">
      <label>{{ t("otExclusions") }}</label>
      <input v-model="exclusionsText" class="wide" placeholder="192.168.1.254" :disabled="scanner.isRunning" />
    </div>
    <div class="hint">{{ t("otAllowlistHint") }}</div>

    <div class="form-row">
      <label>{{ t("probeFcs") }}</label>
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
      <label>{{ t("stopFirst") }}</label>
      <input type="checkbox" v-model="params.stop_on_first" :disabled="scanner.isRunning" />
      <label class="short">{{ t("identify") }}</label>
      <input type="checkbox" v-model="params.identify_devices" :disabled="scanner.isRunning" />
      <span class="hint">{{ t("identifyHint") }}</span>
    </div>

    <div class="form-row">
      <label>{{ t("tcpAdu") }}</label>
      <select v-model="tcpFraming" :disabled="scanner.isRunning">
        <option value="mbap">{{ t("mbap") }}</option>
        <option value="rtu">{{ t("rtuOverTcp") }}</option>
      </select>
      <label class="short">{{ t("udpMbap") }}</label>
      <input type="checkbox" v-model="params.udp_mbap" :disabled="scanner.isRunning" />
      <span class="hint">{{ t("tcpAduHint") }}</span>
    </div>

    <div class="form-row">
      <label>{{ t("tlsTitle") }}</label>
      <input type="checkbox" v-model="tlsEnabled" :disabled="scanner.isRunning" />
      <span class="hint">{{ t("tlsHint") }}</span>
    </div>

    <div v-if="tlsEnabled" class="form-row">
      <label>{{ t("tlsCert") }}</label>
      <input v-model="params.tls!.client_cert" class="wide" placeholder="client.pem" :disabled="scanner.isRunning" />
      <label class="short">{{ t("tlsKey") }}</label>
      <input v-model="params.tls!.client_key" class="wide" placeholder="client.key" :disabled="scanner.isRunning" />
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

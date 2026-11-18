<script setup lang="ts">
import { reactive } from "vue";

import ScanFormSerial from "@/components/scan/ScanFormSerial.vue";
import ScanFormIp from "@/components/scan/ScanFormIp.vue";
import ScanParamsForm from "@/components/scan/ScanParamsForm.vue";
import ScanControls from "@/components/scan/ScanControls.vue";
import RegisterScanForm from "@/components/scan/RegisterScanForm.vue";
import LogPanel from "@/components/common/LogPanel.vue";
import type { ScanParams, ScanTarget } from "@/types/ipc";
import { useScannerStore } from "@/stores/scanner";
import { useSettingsStore } from "@/stores/settings";

const scanner = useScannerStore();
const settings = useSettingsStore();

/** Local editable copy of scan params (saved back to config on start). */
const params = reactive<ScanParams>(JSON.parse(JSON.stringify(settings.config.scan_params)));

async function start(target: ScanTarget) {
  try {
    settings.config.scan_params = JSON.parse(JSON.stringify(params));
    await settings.save();
    await scanner.start(target, params);
  } catch (e) {
    scanner.addLog("error", `start failed: ${e}`);
  }
}
</script>

<template>
  <div class="scan-view">
    <ScanControls />

    <div class="scan-grid">
      <div class="scan-forms">
        <ScanFormSerial :params="params" @start="start" />
        <ScanFormIp :params="params" @start="start" />
        <RegisterScanForm @start="start" />
      </div>
      <div class="scan-params">
        <ScanParamsForm v-model="params" />
      </div>
    </div>

    <LogPanel />
  </div>
</template>

<style scoped>
.scan-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

@media (max-width: 1100px) {
  .scan-grid {
    grid-template-columns: 1fr;
  }
}
</style>

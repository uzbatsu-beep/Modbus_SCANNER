<script setup lang="ts">
import { onMounted } from "vue";

import DevicesTable from "@/components/results/DevicesTable.vue";
import DeviceDetail from "@/components/results/DeviceDetail.vue";
import IpTargetTable from "@/components/results/IpTargetTable.vue";
import { useDevicesStore } from "@/stores/devices";
import { useScannerStore } from "@/stores/scanner";

const devices = useDevicesStore();
const scanner = useScannerStore();

onMounted(() => {
  devices.refresh().catch(() => {});
  scanner.addLog("info", "results view opened");
});
</script>

<template>
  <div class="results-view">
    <DevicesTable />
    <IpTargetTable />
    <DeviceDetail />
  </div>
</template>

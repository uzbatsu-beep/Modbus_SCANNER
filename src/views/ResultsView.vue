<script setup lang="ts">
import { onActivated } from "vue";
import {t} from "@/i18n";
import {useWorkbenchStore} from "@/stores/workbench";
defineOptions({name:"ResultsView"});

import DevicesTable from "@/components/results/DevicesTable.vue";
import DeviceDetail from "@/components/results/DeviceDetail.vue";
import IpTargetTable from "@/components/results/IpTargetTable.vue";
import MonitorPanel from "@/components/results/MonitorPanel.vue";
import { useDevicesStore } from "@/stores/devices";
import { useScannerStore } from "@/stores/scanner";

const devices = useDevicesStore();
const scanner = useScannerStore();
const wb=useWorkbenchStore();
onActivated(()=>{devices.refresh().catch(e=>scanner.addLog('error',String(e)));});


</script>

<template>
  <div class="results-view">
    <nav class="wb-tabs" :aria-label="t('navResults')">
      <button :aria-pressed="wb.resultTab==='devices'" :class="{active:wb.resultTab==='devices'}" @click="wb.resultTab='devices'">{{ t("devicesTitle") }}</button>
      <button :aria-pressed="wb.resultTab==='registers'" :class="{active:wb.resultTab==='registers'}" @click="wb.resultTab='registers'">{{ t("wbOpenRegisters") }}</button>
      <button :aria-pressed="wb.resultTab==='monitor'" :class="{active:wb.resultTab==='monitor'}" @click="wb.resultTab='monitor'">{{ t("monitorTitle") }}</button>
    </nav>
    <div v-show="wb.resultTab==='devices'"><DevicesTable /><IpTargetTable /></div>
    <div v-show="wb.resultTab==='registers'"><DeviceDetail /></div>
    <div v-show="wb.resultTab==='monitor'"><MonitorPanel /></div>
  </div>
</template>

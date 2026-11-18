<script setup lang="ts">
import { onMounted } from "vue";

import TopBar from "@/components/layout/TopBar.vue";
import StatusBar from "@/components/layout/StatusBar.vue";
import { useScannerStore } from "@/stores/scanner";
import { useSettingsStore } from "@/stores/settings";
import { useMonitorStore } from "@/stores/monitor";
import { useTrafficStore } from "@/stores/traffic";

const scanner = useScannerStore();
const settings = useSettingsStore();
const monitor = useMonitorStore();
const traffic = useTrafficStore();

onMounted(async () => {
  await settings.load();
  await scanner.bindEvents();
  await monitor.bind();
  await monitor.refresh();
  await monitor.refreshLoggers();
  await traffic.bind();
});
</script>

<template>
  <div class="layout">
    <TopBar />
    <div class="layout-main">
      <router-view class="layout-content" />
    </div>
    <StatusBar />
  </div>
</template>

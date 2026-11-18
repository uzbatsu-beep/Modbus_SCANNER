<script setup lang="ts">
import { onMounted } from "vue";

import TopBar from "@/components/layout/TopBar.vue";
import StatusBar from "@/components/layout/StatusBar.vue";
import { useScannerStore } from "@/stores/scanner";
import { useSettingsStore } from "@/stores/settings";
import { useMonitorStore } from "@/stores/monitor";

const scanner = useScannerStore();
const settings = useSettingsStore();
const monitor = useMonitorStore();

onMounted(async () => {
  await settings.load();
  await scanner.bindEvents();
  await monitor.bind();
  await monitor.refresh();
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

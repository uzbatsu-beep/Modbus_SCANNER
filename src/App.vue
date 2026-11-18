<script setup lang="ts">
import { onMounted, watch } from "vue";

import TopBar from "@/components/layout/TopBar.vue";
import StatusBar from "@/components/layout/StatusBar.vue";
import { useScannerStore } from "@/stores/scanner";
import { useSettingsStore } from "@/stores/settings";
import { useMonitorStore } from "@/stores/monitor";
import { useTrafficStore } from "@/stores/traffic";
import { useLocaleStore } from "@/stores/locale";

const scanner = useScannerStore();
const settings = useSettingsStore();
const monitor = useMonitorStore();
const traffic = useTrafficStore();
const locale = useLocaleStore();

function applyTheme() {
  document.documentElement.dataset.theme = settings.config.theme === "light" ? "light" : "dark";
}

watch(
  () => settings.config.language,
  (lang) => locale.set(lang),
  { immediate: true }
);
watch(
  () => settings.config.theme,
  () => applyTheme(),
  { immediate: true }
);

onMounted(async () => {
  await settings.load();
  locale.set(settings.config.language);
  applyTheme();
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

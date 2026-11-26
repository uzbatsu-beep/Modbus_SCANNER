<script setup lang="ts">
import { onMounted, onBeforeUnmount, watch, computed, ref } from "vue";
import { useWorkbenchStore } from "@/stores/workbench";
import { fitLayout } from "@/types/workbenchLayout";
import { useDevicesStore } from "@/stores/devices";
import ProjectExplorer from "@/components/layout/ProjectExplorer.vue";
import ContextInspector from "@/components/layout/ContextInspector.vue";
import BottomDock from "@/components/layout/BottomDock.vue";
import PanelSplitter from "@/components/layout/PanelSplitter.vue";
import { t } from "@/i18n";

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
const wb = useWorkbenchStore();
const devices = useDevicesStore();
const ready = ref(false);
const projectGeneration = ref(0);
watch(()=>settings.loadedProject,()=>{projectGeneration.value++;wb.resultTab='devices';});
const shell = ref<HTMLElement|null>(null);
const width = ref(1024), height = ref(640);
const fitted = computed(()=>fitLayout(wb.layout,width.value,height.value));
let observer: ResizeObserver | undefined;
let persistTimer: ReturnType<typeof setTimeout> | undefined;
watch(()=>wb.layout,()=>{if(persistTimer!==undefined) clearTimeout(persistTimer);persistTimer=setTimeout(()=>wb.persist(),150);},{deep:true});
const persistOnExit=()=>wb.persist();
onBeforeUnmount(()=>{observer?.disconnect();if(persistTimer!==undefined)clearTimeout(persistTimer);wb.persist();window.removeEventListener('beforeunload',persistOnExit);});

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
  ready.value = true;
  if(shell.value){observer=new ResizeObserver(([entry])=>{width.value=entry.contentRect.width;height.value=entry.contentRect.height;});observer.observe(shell.value);}
  window.addEventListener('beforeunload',persistOnExit);
  // Bind event sources before the initial snapshot, and isolate startup failures.
  await scanner.bindEvents().catch(e=>scanner.addLog("error",String(e)));
  await monitor.bind().catch(e=>scanner.addLog("error",String(e)));
  await traffic.bind().catch(e=>scanner.addLog("error",String(e)));
  await devices.refresh().catch(e=>scanner.addLog("error",String(e)));
  await monitor.refresh().catch(e=>scanner.addLog("error",String(e)));
  await monitor.refreshLoggers().catch(e=>scanner.addLog("error",String(e)));
});
</script>

<template>
  <div class="layout">
    <TopBar />
    <div ref="shell" class="wb-shell" :style="{gridTemplateRows: 'minmax(0,1fr) '+(fitted.bottom?'6px '+fitted.bottom+'px':'0px 0px')}">
      <div class="wb-main" :style="{gridTemplateColumns: fitted.left+'px '+(fitted.left?6:0)+'px minmax(0,1fr) '+(fitted.right?6:0)+'px '+fitted.right+'px'}">
        <aside v-show="fitted.left>0" class="wb-side"><ProjectExplorer /></aside>
        <PanelSplitter v-if="fitted.left" panel="left" :value="fitted.left" :label="t('wbResizeLeft')" @resize="wb.resize('left',$event)" @finish="wb.persist()" />
        <main class="wb-workspace">
          <router-view v-if="ready" v-slot="{Component,route}">
            <KeepAlive :key="projectGeneration" include="ResultsView,ScanView">
              <component :is="Component" :key="route.name" class="wb-page" />
            </KeepAlive>
          </router-view>
        </main>
        <PanelSplitter v-if="fitted.right" panel="right" :value="fitted.right" :label="t('wbResizeRight')" @resize="wb.resize('right',$event)" @finish="wb.persist()" />
        <aside v-show="fitted.right>0" class="wb-side"><ContextInspector /></aside>
      </div>
      <PanelSplitter v-if="fitted.bottom" panel="bottom" :value="fitted.bottom" :label="t('wbResizeBottom')" @resize="wb.resize('bottom',$event)" @finish="wb.persist()" />
      <section v-show="fitted.bottom>0" class="wb-dock"><BottomDock /></section>
    </div>
    <StatusBar />
  </div>
</template>

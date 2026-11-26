<script setup lang="ts">
import { computed,ref } from "vue";
import { useRoute,useRouter } from "vue-router";
import { t,type TranslationKey } from "@/i18n";
import { useDevicesStore } from "@/stores/devices";
import { useScannerStore } from "@/stores/scanner";
import { useMonitorStore } from "@/stores/monitor";
import { useWorkbenchStore } from "@/stores/workbench";
import { useSettingsStore } from "@/stores/settings";
const route=useRoute(), router=useRouter(), devices=useDevicesStore(), scanner=useScannerStore(), monitor=useMonitorStore(), wb=useWorkbenchStore(), settings=useSettingsStore();
const commandBusy=ref(false);
const links=computed(()=>[{to:"/",label:t("navScan")},{to:"/results",label:t("navResults")},{to:"/profiles",label:t("navProfiles")},{to:"/diff",label:t("navDiff")},{to:"/traffic",label:t("navTraffic")},{to:"/manual",label:t("navManual")},{to:"/settings",label:t("navSettings")}]);
function close(e:MouseEvent){const menu=(e.currentTarget as HTMLElement).closest("details");if(menu)menu.open=false;}
async function command(action:()=>Promise<unknown>) {
 if(commandBusy.value)return;commandBusy.value=true;
 try{await action();}catch(e){scanner.addLog("error",String(e));}finally{commandBusy.value=false;}
}
function openMonitor(){wb.resultTab="monitor";void router.push("/results");}
function openRegisters(){wb.resultTab="registers";void router.push("/results");}
const panels=computed(()=>[{key:"left" as const,label:t("wbExplorer"),visible:wb.layout.leftVisible},{key:"right" as const,label:t("wbInspector"),visible:wb.layout.rightVisible},{key:"bottom" as const,label:t("wbBottom"),visible:wb.layout.bottomVisible}]);
const themeLabel=computed<TranslationKey>(()=>settings.config.theme==="dark"?"light":"dark");
async function theme(){const previous=settings.config.theme;settings.config.theme=previous==="dark"?"light":"dark";try{await settings.save();}catch(e){settings.config.theme=previous;throw e;}}
</script>
<template>
  <header class="wb-header">
    <div class="wb-menubar">
      <strong class="wb-brand">Modbus Scanner</strong>
      <details class="wb-menu"><summary>{{ t("wbFile") }}</summary><div class="wb-menu-list"><router-link to="/settings" @click="close">{{ t("wbProjectFiles") }}</router-link></div></details>
      <details class="wb-menu"><summary>{{ t("project") }}</summary><div class="wb-menu-list"><router-link to="/" @click="close">{{ t("navScan") }}</router-link><router-link to="/results" @click="close">{{ t("navResults") }}</router-link><router-link to="/profiles" @click="close">{{ t("navProfiles") }}</router-link></div></details>
      <details class="wb-menu"><summary>{{ t("wbView") }}</summary><div class="wb-menu-list"><button v-for="p in panels" :key="p.key" :aria-pressed="p.visible" @click="wb.toggle(p.key);close($event)">{{ p.visible?'✓':'○' }} {{ p.label }}</button><button @click="wb.reset();close($event)">{{ t("wbResetLayout") }}</button><button :disabled="settings.saving || commandBusy" @click="command(theme);close($event)">{{ t("theme") }}: {{ t(themeLabel) }}</button></div></details>
      <details class="wb-menu"><summary>{{ t("wbTools") }}</summary><div class="wb-menu-list"><router-link to="/manual" @click="close">{{ t("navManual") }}</router-link><router-link to="/traffic" @click="close">{{ t("navTraffic") }}</router-link><router-link to="/diff" @click="close">{{ t("navDiff") }}</router-link></div></details>
      <details class="wb-menu"><summary>{{ t("wbHelp") }}</summary><div class="wb-menu-list"><router-link to="/settings" @click="close">{{ t("wbAbout") }}</router-link></div></details>
      <span class="wb-spacer"/><span class="wb-muted wb-brand-sub">RTU / ASCII / TCP / UDP</span>
    </div>
    <nav class="wb-commandbar" :aria-label="t('wbCommands')">
      <router-link v-for="l in links" :key="l.to" :to="l.to" class="wb-command" :class="{active:route.path===l.to}">{{ l.label }}<span v-if="l.to==='/results' && devices.foundCount"> ({{ devices.foundCount }})</span></router-link>
      <span class="wb-command-divider"/>
      <button :disabled="!devices.selected" @click="openRegisters">{{ t("wbOpenRegisters") }}</button>
      <button @click="openMonitor">{{ t("wbOpenMonitor") }}</button>
      <button v-if="scanner.isRunning" :disabled="commandBusy" @click="command(()=>scanner.pause())">{{ t("pause") }}</button>
      <button v-if="scanner.isPaused" :disabled="commandBusy" @click="command(()=>scanner.resume())">{{ t("resume") }}</button>
      <button v-if="!scanner.canStart" class="danger" :disabled="commandBusy || scanner.status==='stopping'" @click="command(()=>scanner.stop())">{{ t("wbStopScan") }}</button>
      <button v-if="monitor.running" class="danger" :disabled="monitor.busy || commandBusy" @click="command(()=>monitor.stop())">{{ t("wbStopMonitor") }}</button>
      <span class="wb-spacer"/>
      <button v-for="p in panels" :key="p.key" :aria-pressed="p.visible" :title="p.label" @click="wb.toggle(p.key)">{{ p.key==='left'?'◧':p.key==='right'?'◨':'▤' }}</button>
    </nav>
  </header>
</template>

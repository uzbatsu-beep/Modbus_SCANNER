<script setup lang="ts">
import {computed} from "vue";
import {t} from "@/i18n";
import {useWorkbenchStore} from "@/stores/workbench";
import {useScannerStore} from "@/stores/scanner";
import {useTrafficStore} from "@/stores/traffic";
import {useMonitorStore} from "@/stores/monitor";
const wb=useWorkbenchStore(), scanner=useScannerStore(), traffic=useTrafficStore(), monitor=useMonitorStore();
const events=computed(()=>scanner.log.slice(-100).reverse());
const frames=computed(()=>traffic.records.slice(-100).reverse());
const tabs=computed(()=>[{key:"events" as const,label:t("wbEvents"),count:scanner.log.length},{key:"traffic" as const,label:t("navTraffic"),count:traffic.count},{key:"loggers" as const,label:t("loggers"),count:monitor.loggers.length}]);
</script>
<template>
  <div class="wb-panel-head">
    <button v-for="tab in tabs" :key="tab.key" :aria-pressed="wb.layout.bottomTab===tab.key" :class="{active:wb.layout.bottomTab===tab.key}" @click="wb.layout.bottomTab=tab.key">{{ tab.label }} ({{ tab.count }})</button>
    <span class="wb-spacer"/>
    <router-link v-if="wb.layout.bottomTab==='traffic'" to="/traffic" class="wb-link">{{ t("wbOpenFull") }}</router-link>
    <button v-if="wb.layout.bottomTab==='loggers'" @click="monitor.refreshLoggers().catch(e=>scanner.addLog('error',String(e)))">{{ t("update") }}</button>
    <button :title="t('wbHideBottom')" :aria-label="t('wbHideBottom')" @click="wb.toggle('bottom')">−</button>
  </div>
  <div class="wb-panel-body dock-body">
    <template v-if="wb.layout.bottomTab==='events'">
      <p v-if="!events.length" class="wb-muted">{{ t("logEmpty") }}</p>
      <div v-for="(entry,i) in events" :key="i" class="log-entry" :class="entry.level"><span class="time">{{ entry.time.toLocaleTimeString() }}</span> <span class="msg">{{ entry.level.toUpperCase() }} · {{ entry.message }}</span></div>
    </template>
    <template v-else-if="wb.layout.bottomTab==='traffic'">
      <p v-if="!frames.length" class="wb-muted">{{ t("noTraffic") }}</p>
      <div v-for="r in frames" :key="r.id" class="wb-frame"><span>{{ new Date(r.timestamp_ms).toLocaleTimeString() }} · {{ r.kind.toUpperCase() }} {{ r.endpoint }} / {{ r.unit_id }} · {{ r.status }}</span><span>TX {{ r.tx_hex }} · RX {{ r.rx_hex ?? "—" }}</span></div>
    </template>
    <template v-else>
      <p v-if="!monitor.loggers.length" class="wb-muted">{{ t("noData") }}</p>
      <div v-for="l in monitor.loggers" :key="l.id" class="wb-frame"><strong>{{ l.id }} · {{ l.enabled?t("enabled"):"OFF" }}</strong><span>{{ l.records_written }} {{ t("records") }} · {{ t("loggerQueue") }}: {{ l.pending_records }}/{{ l.dropped_records }}/{{ l.failed_records }} · {{ l.error ?? l.current_file }}</span></div>
    </template>
  </div>
</template>

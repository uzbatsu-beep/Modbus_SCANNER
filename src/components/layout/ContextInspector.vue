<script setup lang="ts">
import {computed} from "vue";
import {t} from "@/i18n";
import {useDevicesStore} from "@/stores/devices";
import {useSettingsStore} from "@/stores/settings";
import {useWorkbenchStore} from "@/stores/workbench";
import {useMonitorStore} from "@/stores/monitor";
const devices=useDevicesStore(), settings=useSettingsStore(), wb=useWorkbenchStore(), monitor=useMonitorStore();
const selected=computed(()=>devices.selected);
const tags=computed(()=>settings.config.tags.filter(t=>t.source.start.device_key===selected.value?.key));
const staleKeys=computed(()=>new Set(monitor.metrics?.channels.filter(c=>c.stale).map(c=>c.key)??[]));
const readings=computed(()=>monitor.values.filter(v=>v.watch.target.kind===selected.value?.target.kind && v.watch.target.endpoint===selected.value?.target.endpoint && v.watch.target.port===selected.value?.target.port && v.watch.target.unit_id===selected.value?.target.unit_id && JSON.stringify(v.watch.target.serial)===JSON.stringify(selected.value?.target.serial) && v.watch.target.serial_mode===selected.value?.target.serial_mode));
</script>
<template>
  <div class="wb-panel-head"><strong>{{ t("wbInspector") }}</strong><button :title="t('wbHideRight')" :aria-label="t('wbHideRight')" @click="wb.toggle('right')">−</button></div>
  <div class="wb-panel-body">
    <p v-if="!selected" class="wb-muted">{{ t("selectDevice") }}</p>
    <template v-else>
      <strong>{{ selected.target.kind.toUpperCase() }} / Unit {{ selected.target.unit_id }}</strong>
      <dl class="wb-properties">
        <dt>{{ t("address") }}</dt><dd>{{ selected.target.endpoint }}{{ selected.target.port===null?"":":"+selected.target.port }}</dd>
        <dt>{{ t("key") }}</dt><dd>{{ selected.key }}</dd>
        <dt>{{ t("registers") }}</dt><dd>{{ Object.keys(selected.registers).length }}</dd>
        <dt>{{ t("tags") }}</dt><dd>{{ tags.length }}</dd>
        <dt>{{ t("lastSeen") }}</dt><dd>{{ new Date(selected.last_seen_ms).toLocaleString() }}</dd>
      </dl>
      <router-link to="/results" class="wb-link" @click="wb.resultTab='registers'">{{ t("wbOpenRegisters") }}</router-link>
      <h3>{{ t("monitorTitle") }}</h3>
      <p v-if="!readings.length" class="wb-muted">{{ t("noData") }}</p>
      <div v-for="v in readings" :key="v.key" class="wb-reading">
        <small>{{ v.watch.name || v.key }}</small>
        <strong>{{ v.latest?.value?.value ?? v.last_good?.value?.value ?? "—" }} {{ v.watch.unit ?? "" }}</strong>
        <small v-if="v.last_good && (v.quality!=='good' || !monitor.running || staleKeys.has(v.key))" class="wb-muted">{{ t('lastGood') }}</small>
        <span class="wb-muted">{{ v.quality==='good' && (!monitor.running || staleKeys.has(v.key))?'stale':v.quality }}</span>
      </div>
      <router-link to="/results" class="wb-link" @click="wb.resultTab='monitor'">{{ t("wbOpenMonitor") }}</router-link>
    </template>
  </div>
</template>

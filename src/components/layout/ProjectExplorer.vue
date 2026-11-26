<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import { t } from "@/i18n";
import { useDevicesStore } from "@/stores/devices";
import { useSettingsStore } from "@/stores/settings";
import { useWorkbenchStore } from "@/stores/workbench";
import { groupDevices } from "@/types/workbenchLayout";
const devices=useDevicesStore(), settings=useSettingsStore(), wb=useWorkbenchStore(), router=useRouter();
const query=ref(""), closed=ref(new Set<string>()), error=ref("");
const groups=computed(()=>groupDevices(devices.records.filter(d=>[d.key,d.target.endpoint,d.target.kind,String(d.target.unit_id)].some(s=>s.toLowerCase().includes(query.value.trim().toLowerCase())))));
function toggle(key:string) { const next=new Set(closed.value); if(next.has(key)) next.delete(key); else next.add(key); closed.value=next; }
function select(key:string) { devices.select(key); wb.resultTab="registers"; void router.push("/results"); }
async function refresh() { error.value=""; try {await devices.refresh();} catch(e){error.value=String(e);} }
</script>
<template>
  <div class="wb-panel-head"><strong>{{ t("wbExplorer") }}</strong><button :aria-label="t('update')" :title="t('update')" @click="refresh">↻</button><button :aria-label="t('wbHideLeft')" :title="t('wbHideLeft')" @click="wb.toggle('left')">−</button></div>
  <div class="wb-panel-body explorer">
    <div class="wb-project-name">{{ settings.loadedProject?.name || t("currentSession") }}</div>
    <input v-model="query" :placeholder="t('wbSearchDevices')" :aria-label="t('wbSearchDevices')" />
    <p class="wb-muted">{{ t("wbDiscoveredGroups") }} ({{ devices.foundCount }})</p>
    <div v-if="error" role="alert">{{ error }}</div>
    <p v-if="!groups.length" class="wb-muted">{{ t("noData") }}</p>
    <div v-for="g in groups" :key="g.key" class="wb-device-group">
      <button class="wb-tree-item" :aria-expanded="!closed.has(g.key)" @click="toggle(g.key)">{{ closed.has(g.key)?'▸':'▾' }} {{ g.label }}</button>
      <div v-show="!closed.has(g.key)" class="wb-device-children">
        <button v-for="d in g.records" :key="d.key" class="wb-tree-item" :class="{selected:d.key===devices.selectedKey}" :aria-pressed="d.key===devices.selectedKey" :title="d.key" @click="select(d.key)">
          <span>Unit {{ d.target.unit_id }}</span><small>{{ Object.keys(d.registers).length }} {{ t("registers") }}</small>
        </button>
      </div>
    </div>
    <hr />
    <router-link to="/profiles" class="wb-tree-item">{{ t("navProfiles") }} ({{ settings.config.profiles.length }})</router-link>
    <router-link to="/diff" class="wb-tree-item">{{ t("navDiff") }}</router-link>
    <router-link to="/settings" class="wb-tree-item">{{ t("project") }}</router-link>
  </div>
</template>

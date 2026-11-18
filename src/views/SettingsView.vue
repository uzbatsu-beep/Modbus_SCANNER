<script setup lang="ts">
import { t } from "@/i18n";
import { reactive, ref } from "vue";

import { api } from "@/api/mod";
import { useSettingsStore } from "@/stores/settings";
import { useScannerStore } from "@/stores/scanner";
import { useDevicesStore } from "@/stores/devices";

const settings = useSettingsStore();
const scanner = useScannerStore();

const local = reactive({
  language: settings.config.language,
  theme: settings.config.theme,
  export_format: settings.config.export_format,
  export_dir: settings.config.export_dir ?? "",
});

const saving = ref(false);
const msg = ref<string | null>(null);

const devices = useDevicesStore();
const project = reactive({ name: "Modbus scan", path: "modbus-project.json" });
const projectMsg = ref<string | null>(null);

async function saveProject() {
  projectMsg.value = null;
  try {
    await api.scanner.saveProject(project.path.trim(), project.name.trim() || "Modbus scan");
    projectMsg.value = t("projectSaved");
  } catch (e) {
    projectMsg.value = `${t("errorPrefix")} ${e}`;
  }
}

async function loadProject() {
  projectMsg.value = null;
  try {
    const p = await api.scanner.loadProject(project.path.trim());
    settings.loadedProject = p;
    project.name = p.name;
    await settings.load();
    await devices.refresh();
    projectMsg.value = `${t("projectLoaded")}: ${p.results.records.length}, ${t("tags")}: ${p.tags.length}, ${t("snapshots")}: ${p.history.length}`;
  } catch (e) {
    projectMsg.value = `${t("errorPrefix")} ${e}`;
  }
}
const appInfo = ref<{ name: string; version: string; debug: boolean } | null>(null);

api.system.getAppInfo().then((i) => (appInfo.value = i)).catch(() => {});

async function save() {
  saving.value = true;
  msg.value = null;
  try {
    settings.config.language = local.language;
    settings.config.theme = local.theme;
    settings.config.export_format = local.export_format;
    settings.config.export_dir = local.export_dir || null;
    await settings.save();
    msg.value = t("settingsSaved");
  } catch (e) {
    msg.value = `${t("errorPrefix")} ${e}`;
  } finally {
    saving.value = false;
  }
}

function clearLog() {
  scanner.log = [];
}
</script>

<template>
  <div class="settings-view">
    <div class="panel">
      <div class="panel-title">{{ t("general") }}</div>

      <div class="form-row">
        <label>{{ t("language") }}</label>
        <select v-model="local.language">
          <option value="ru">Русский</option>
          <option value="en">English</option>
        </select>
      </div>

      <div class="form-row">
        <label>{{ t("theme") }}</label>
        <select v-model="local.theme">
          <option value="dark">{{ t("dark") }}</option>
          <option value="light">{{ t("light") }}</option>
        </select>
      </div>
    </div>

    <div class="panel">
      <div class="panel-title">{{ t("exportSettings") }}</div>

      <div class="form-row">
        <label>{{ t("defaultFormat") }}</label>
        <select v-model="local.export_format">
          <option value="csv">CSV</option>
          <option value="json">JSON</option>
          <option value="xml">XML</option>
        </select>
      </div>

      <div class="form-row">
        <label>{{ t("exportDir") }}</label>
        <input v-model="local.export_dir" placeholder="Documents/modbus-scan" class="wide" />
      </div>
    </div>

    <div class="panel">
      <div class="panel-title">{{ t("project") }}</div>

      <div class="form-row">
        <label>{{ t("projectName") }}</label>
        <input v-model="project.name" placeholder="Modbus scan" />
      </div>

      <div class="form-row">
        <label>{{ t("projectFile") }}</label>
        <input v-model="project.path" class="wide" placeholder="modbus-project.json" />
      </div>

      <div class="form-row">
        <label />
        <button :disabled="!project.path.trim()" @click="saveProject">{{ t("saveProject") }}</button>
        <button :disabled="!project.path.trim()" @click="loadProject">{{ t("loadProject") }}</button>
        <span v-if="projectMsg" class="msg">{{ projectMsg }}</span>
      </div>

      <div v-if="settings.loadedProject" class="form-row">
        <label>{{ t("loaded") }}</label>
        <span class="mono">
          {{ settings.loadedProject.name }} ·
          {{ settings.loadedProject.results.records.length }} {{ t("devicesTitle") }} ·
          {{ settings.loadedProject.tags.length }} {{ t("tags") }} ·
          {{ settings.loadedProject.history.length }} {{ t("snapshots") }}
        </span>
      </div>
    </div>

    <div class="panel">
      <div class="panel-title">{{ t("diagnostics") }}</div>

      <div class="form-row">
        <label>{{ t("version") }}</label>
        <span v-if="appInfo" class="mono">
          {{ appInfo.name }} v{{ appInfo.version }} {{ appInfo.debug ? "(debug)" : "(release)" }}
        </span>
      </div>

      <div class="form-row">
        <label>{{ t("logLevel") }}</label>
        <span class="mono">{{ "info,modbus_scanner_lib=debug" }}</span>
      </div>

      <div class="form-row">
        <label>{{ t("uiLog") }}</label>
        <span class="mono">{{ scanner.log.length }} {{ t("records") }}</span>
        <button @click="clearLog()">{{ t("clear") }}</button>
      </div>
    </div>

    <div class="form-row">
      <button class="primary" :disabled="saving" @click="save">{{ t("saveSettings") }}</button>
      <span v-if="msg" class="msg">{{ msg }}</span>
    </div>
  </div>
</template>

<style scoped>
.wide {
  min-width: 320px;
}

.msg {
  font-family: var(--mono);
  font-size: 12px;
  color: var(--fg-dim);
}
</style>

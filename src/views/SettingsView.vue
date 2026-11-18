<script setup lang="ts">
import { reactive, ref } from "vue";

import { api } from "@/api/mod";
import type { Project } from "@/types/ipc";
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
const loadedProject = ref<Project | null>(null);
const projectMsg = ref<string | null>(null);

async function saveProject() {
  projectMsg.value = null;
  try {
    await api.scanner.saveProject(project.path.trim(), project.name.trim() || "Modbus scan");
    projectMsg.value = "проект сохранён";
  } catch (e) {
    projectMsg.value = `ошибка: ${e}`;
  }
}

async function loadProject() {
  projectMsg.value = null;
  try {
    const p = await api.scanner.loadProject(project.path.trim());
    loadedProject.value = p;
    project.name = p.name;
    await settings.load();
    await devices.refresh();
    projectMsg.value = `загружено устройств: ${p.results.records.length}, тегов: ${p.tags.length}, снимков: ${p.history.length}`;
  } catch (e) {
    projectMsg.value = `ошибка: ${e}`;
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
    msg.value = "настройки сохранены";
  } catch (e) {
    msg.value = `ошибка: ${e}`;
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
      <div class="panel-title">Общие / General</div>

      <div class="form-row">
        <label>Язык / Language</label>
        <select v-model="local.language">
          <option value="ru">Русский</option>
          <option value="en">English</option>
        </select>
      </div>

      <div class="form-row">
        <label>Тема / Theme</label>
        <select v-model="local.theme">
          <option value="dark">Dark</option>
          <option value="light">Light (в разработке)</option>
        </select>
      </div>
    </div>

    <div class="panel">
      <div class="panel-title">Экспорт / Export</div>

      <div class="form-row">
        <label>Формат по умолчанию</label>
        <select v-model="local.export_format">
          <option value="csv">CSV</option>
          <option value="json">JSON</option>
          <option value="xml">XML</option>
        </select>
      </div>

      <div class="form-row">
        <label>Каталог по умолчанию</label>
        <input v-model="local.export_dir" placeholder="Documents/modbus-scan" class="wide" />
      </div>
    </div>

    <div class="panel">
      <div class="panel-title">Проект / Project (устройства + теги + история)</div>

      <div class="form-row">
        <label>Название</label>
        <input v-model="project.name" placeholder="Modbus scan" />
      </div>

      <div class="form-row">
        <label>Файл проекта</label>
        <input v-model="project.path" class="wide" placeholder="modbus-project.json" />
      </div>

      <div class="form-row">
        <label />
        <button :disabled="!project.path.trim()" @click="saveProject">Сохранить проект</button>
        <button :disabled="!project.path.trim()" @click="loadProject">Загрузить проект</button>
        <span v-if="projectMsg" class="msg">{{ projectMsg }}</span>
      </div>

      <div v-if="loadedProject" class="form-row">
        <label>Загружен</label>
        <span class="mono">
          {{ loadedProject.name }} · {{ loadedProject.results.records.length }} устройств ·
          {{ loadedProject.tags.length }} тегов · {{ loadedProject.history.length }} снимков
        </span>
      </div>
    </div>

    <div class="panel">
      <div class="panel-title">Диагностика / Diagnostics</div>

      <div class="form-row">
        <label>Версия</label>
        <span v-if="appInfo" class="mono">
          {{ appInfo.name }} v{{ appInfo.version }} {{ appInfo.debug ? "(debug)" : "(release)" }}
        </span>
      </div>

      <div class="form-row">
        <label>Уровень логов (RUST_LOG)</label>
        <span class="mono">{{ "info,modbus_scanner_lib=debug" }}</span>
      </div>

      <div class="form-row">
        <label>Журнал UI</label>
        <span class="mono">{{ scanner.log.length }} записей</span>
        <button @click="clearLog()">Очистить</button>
      </div>
    </div>

    <div class="form-row">
      <button class="primary" :disabled="saving" @click="save">Сохранить настройки</button>
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

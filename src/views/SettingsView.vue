<script setup lang="ts">
import { reactive, ref } from "vue";

import { api } from "@/api/mod";
import { useSettingsStore } from "@/stores/settings";
import { useScannerStore } from "@/stores/scanner";

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

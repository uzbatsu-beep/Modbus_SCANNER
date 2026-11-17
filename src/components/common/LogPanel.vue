<script setup lang="ts">
import { useScannerStore } from "@/stores/scanner";

const scanner = useScannerStore();
</script>

<template>
  <div class="panel">
    <div class="panel-title">Журнал / Log</div>
    <div class="log-panel">
      <div
        v-for="(entry, i) in scanner.log"
        :key="i"
        class="log-entry"
        :class="entry.level"
      >
        <span class="time">{{ entry.time.toLocaleTimeString() }}</span>
        <span class="msg">{{ entry.message }}</span>
      </div>
      <div v-if="scanner.log.length === 0" class="log-entry dim">— пусто —</div>
    </div>
  </div>
</template>

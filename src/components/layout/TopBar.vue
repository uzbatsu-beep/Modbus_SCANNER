<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";

import { useDevicesStore } from "@/stores/devices";

const route = useRoute();
const devices = useDevicesStore();

const links = [
  { to: "/", label: "Сканер / Scanner" },
  { to: "/results", label: "Результаты / Results" },
  { to: "/diff", label: "Diff" },
  { to: "/traffic", label: "Трафик / Traffic" },
  { to: "/manual", label: "Ручной режим / Manual" },
  { to: "/settings", label: "Настройки / Settings" },
];

const foundBadge = computed(() => (devices.foundCount > 0 ? devices.foundCount : ""));
</script>

<template>
  <header class="topbar">
    <div class="brand">
      <span class="brand-name">Modbus Scanner</span>
      <span class="brand-sub">RTU / ASCII / TCP / UDP</span>
    </div>
    <nav class="nav">
      <router-link
        v-for="l in links"
        :key="l.to"
        :to="l.to"
        class="nav-link"
        :class="{ active: route.path === l.to }"
      >
        {{ l.label }}
        <span v-if="l.to === '/results' && foundBadge" class="badge">{{ foundBadge }}</span>
      </router-link>
    </nav>
  </header>
</template>

<style scoped>
.topbar {
  display: flex;
  align-items: center;
  gap: 32px;
  background: var(--bg-panel);
  border-bottom: 1px solid var(--border);
  padding: 0 16px;
  height: 46px;
  flex-shrink: 0;
}

.brand {
  display: flex;
  flex-direction: column;
  line-height: 1.1;
}

.brand-name {
  font-weight: 700;
  font-size: 14px;
}

.brand-sub {
  font-size: 10px;
  color: var(--fg-dim);
  font-family: var(--mono);
}

.nav {
  display: flex;
  gap: 4px;
  height: 100%;
}

.nav-link {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 14px;
  color: var(--fg-dim);
  text-decoration: none;
  font-size: 13px;
  border-bottom: 2px solid transparent;
}

.nav-link:hover {
  color: var(--fg);
}

.nav-link.active {
  color: var(--accent);
  border-bottom-color: var(--accent);
}

.badge {
  background: var(--accent);
  color: #fff;
  border-radius: 8px;
  font-size: 11px;
  padding: 0 6px;
  font-family: var(--mono);
}
</style>

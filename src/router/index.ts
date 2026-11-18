/**
 * Router: main layout tabs.
 */

import { createRouter, createWebHistory } from "vue-router";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/",
      name: "scan",
      component: () => import("@/views/ScanView.vue"),
    },
    {
      path: "/results",
      name: "results",
      component: () => import("@/views/ResultsView.vue"),
    },
    {
      path: "/traffic",
      name: "traffic",
      component: () => import("@/views/TrafficView.vue"),
    },
    {
      path: "/manual",
      name: "manual",
      component: () => import("@/views/ManualView.vue"),
    },
    {
      path: "/settings",
      name: "settings",
      component: () => import("@/views/SettingsView.vue"),
    },
  ],
});

export default router;

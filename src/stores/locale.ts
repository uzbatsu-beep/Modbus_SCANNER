/** Current UI language (P3-09). Kept separate from settings to avoid cycles. */

import { defineStore } from "pinia";

export type Lang = "ru" | "en";

export const useLocaleStore = defineStore("locale", {
  state: () => ({
    lang: "ru" as Lang,
  }),

  actions: {
    set(lang: string) {
      this.lang = lang === "en" ? "en" : "ru";
      document.documentElement.lang = this.lang;
    },
  },
});

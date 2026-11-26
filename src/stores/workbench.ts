import { defineStore } from "pinia";
import { DEFAULT_LAYOUT, LAYOUT_KEY, clampPanel, parseLayout, type Panel, type ResultTab } from "@/types/workbenchLayout";

function initial() {
  try { return parseLayout(localStorage.getItem(LAYOUT_KEY)); }
  catch { return {...DEFAULT_LAYOUT}; }
}
export const useWorkbenchStore = defineStore("workbench", {
  state: () => ({ layout: initial(), resultTab: "devices" as ResultTab, storageError: false }),
  actions: {
    resize(panel:Panel, value:number) { this.layout[panel]=clampPanel(panel,value); },
    toggle(panel:Panel) {
      const key = panel==="left"?"leftVisible":panel==="right"?"rightVisible":"bottomVisible";
      this.layout[key]=!this.layout[key];
    },
    reset() { this.layout={...DEFAULT_LAYOUT}; this.persist(); },
    persist() {
      try { localStorage.setItem(LAYOUT_KEY,JSON.stringify(this.layout)); this.storageError=false; }
      catch { this.storageError=true; }
    },
  },
});

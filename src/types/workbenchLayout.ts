import type { DeviceRecord } from "./ipc";

export type Panel = "left" | "right" | "bottom";
export type ResultTab = "devices" | "registers" | "monitor";
export interface LayoutState {
  version: 1;
  left: number;
  right: number;
  bottom: number;
  leftVisible: boolean;
  rightVisible: boolean;
  bottomVisible: boolean;
  bottomTab: "events" | "traffic" | "loggers";
}
export const LAYOUT_KEY = "modbus-scanner.workbench.layout.v1";
export const DEFAULT_LAYOUT: Readonly<LayoutState> = {
  version: 1, left: 220, right: 250, bottom: 180,
  leftVisible: true, rightVisible: true, bottomVisible: true, bottomTab: "events",
};
export const LIMITS = { left: [160, 480], right: [180, 520], bottom: [100, 420] } as const;
export function clampPanel(panel: Panel, value: number): number {
  const [min,max] = LIMITS[panel];
  return Number.isFinite(value) ? Math.round(Math.max(min, Math.min(max,value))) : DEFAULT_LAYOUT[panel];
}
export function parseLayout(raw: string | null): LayoutState {
  const result = {...DEFAULT_LAYOUT};
  try {
    const v = JSON.parse(raw ?? "null");
    if (!v || v.version !== 1) return result;
    for (const p of ["left","right","bottom"] as const) {
      if (typeof v[p] === "number") result[p] = clampPanel(p,v[p]);
    }
    for (const p of ["leftVisible","rightVisible","bottomVisible"] as const) {
      if (typeof v[p] === "boolean") result[p] = v[p];
    }
    if (["events","traffic","loggers"].includes(v.bottomTab)) result.bottomTab = v.bottomTab;
  } catch { /* corrupt local preference must not prevent startup */ }
  return result;
}
export function fitLayout(state: LayoutState, width: number, height: number) {
  const w = Math.max(0,width), h = Math.max(0,height);
  // Reserve a usable center first. Automatic collapse never changes persisted preferences.
  const left = state.leftVisible && w >= 526;
  const right = state.rightVisible && w >= (left ? 712 : 546);
  const budget = Math.max(0,w-360-(left?6:0)-(right?6:0));
  const l = left ? Math.min(state.left, budget-(right?180:0)) : 0;
  const r = right ? Math.min(state.right, budget-l) : 0;
  const bottom = state.bottomVisible && h >= 306;
  return { left: Math.max(0,l), right: Math.max(0,r), bottom: bottom ? Math.min(state.bottom,h-206) : 0 };
}
export function groupDevices(records: readonly DeviceRecord[]) {
  const groups = new Map<string,{ key:string; label:string; records:DeviceRecord[] }>();
  for (const record of records) {
    const {unit_id: _unit, ...transport} = record.target;
    const key = JSON.stringify(transport);
    let group=groups.get(key);
    if (!group) {
      group={key,label:record.target.kind.toUpperCase()+" · "+record.target.endpoint+(record.target.port==null?"":":"+record.target.port),records:[]};
      groups.set(key,group);
    }
    group.records.push(record);
  }
  return [...groups.values()].sort((a,b)=>a.label.localeCompare(b.label)||a.key.localeCompare(b.key)).map(g=>({...g,records:[...g.records].sort((a,b)=>a.target.unit_id-b.target.unit_id||a.key.localeCompare(b.key))}));
}

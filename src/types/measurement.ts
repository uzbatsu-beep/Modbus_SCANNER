import type { MonitorValueState } from "./ipc";
export function mergeChannel(old: MonitorValueState | undefined, next: MonitorValueState): MonitorValueState {
  if (!old || old.session_id !== next.session_id) return next;
  const samples = new Map(old.samples.map(s => [s.sequence, s]));
  for (const s of next.samples) samples.set(s.sequence, s);
  const newest = (old.latest?.sequence ?? 0) > (next.latest?.sequence ?? 0) ? old : next;
  return { ...newest, samples: [...samples.values()].sort((a,b) => a.sequence-b.sequence).slice(-newest.history_capacity) };
}

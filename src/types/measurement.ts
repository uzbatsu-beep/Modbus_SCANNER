import type { MonitorValueState, MonitorSample } from "./ipc";
export function mergeChannel(old: MonitorValueState | undefined, next: MonitorValueState): MonitorValueState {
  if (!old || old.session_id !== next.session_id) return next;
  const last = old.samples[old.samples.length-1]?.sequence ?? 0;
  const newest = (old.latest?.sequence ?? 0) > (next.latest?.sequence ?? 0) ? old : next;
  if (next.samples.length && next.samples[0].sequence > last) {
    return { ...newest, samples: [...old.samples, ...next.samples].slice(-newest.history_capacity) };
  }
  const samples = new Map(old.samples.map(s => [s.sequence, s]));
  for (const s of next.samples) samples.set(s.sequence, s);
  return { ...newest, samples: [...samples.values()].sort((a,b) => a.sequence-b.sequence).slice(-newest.history_capacity) };
}
export function trendSegments(samples: MonitorSample[], end: number, windowMs: number) {
  const start = end - windowMs * 1000;
  const selected = samples.filter(s => s.elapsed_us >= start && s.elapsed_us <= end);
  const numbers = selected.flatMap(s => s.quality === "good" && typeof s.value?.value === "number" ? [s.value.value] : []);
  if (!numbers.length || windowMs <= 0) return null;
  let lo = Math.min(0, ...numbers), hi = Math.max(0, ...numbers);
  if (lo === hi) { lo -= 1; hi += 1; }
  const magnitude = Math.max(Math.abs(lo), Math.abs(hi));
  const y = (v: number) => 120 - (v / magnitude - lo / magnitude) / (hi / magnitude - lo / magnitude) * 120;
  const segments: string[][] = [];
  let current: string[] = [];
  let prev = 0;
  for (const s of selected) {
    if ((prev && s.sequence !== prev + 1) || s.quality !== "good" || typeof s.value?.value !== "number") {
      if (current.length) segments.push(current);
      current = [];
    }
    if (s.quality === "good" && typeof s.value?.value === "number") {
      current.push(`${((s.elapsed_us - start) / (end - start) * 600).toFixed(2)},${y(s.value.value).toFixed(2)}`);
    }
    prev = s.sequence;
  }
  if (current.length) segments.push(current);
  return { segments: segments.map(x => x.join(" ")), zero: y(0), lo, hi };
}

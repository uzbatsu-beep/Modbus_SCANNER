import type { AddressSpace, DataType, RegisterSpan, RegisterValue } from "./ipc";

export function addressSpace(fc: number): AddressSpace | undefined {
  switch (fc) {
    case 1: return "coils";
    case 2: return "discrete_inputs";
    case 3: return "holding_registers";
    case 4: return "input_registers";
    default: return undefined;
  }
}
export function readFunction(space: AddressSpace): number {
  return { coils: 1, discrete_inputs: 2, holding_registers: 3, input_registers: 4 }[space];
}
export function registerKey(fc: number, address: number): string {
  return `${fc}:${address}`;
}
export function registerWidth(type: DataType): number {
  return ["int32", "uint32", "float32"].includes(type) ? 2 : 1;
}
export function readSpan(registers: readonly RegisterValue[], span: RegisterSpan): number[] | null {
  const fc = readFunction(span.start.address_space);
  const count = span.quantity;
  const start = span.start.address;
  if (!Number.isInteger(count) || count < 1 || !Number.isInteger(start) || start < 0 || start + count > 65536) return null;
  const index = new Map(registers.map(r => [registerKey(r.function, r.address), r.raw]));
  const words: number[] = [];
  for (let i = 0; i < count; i++) {
    const value = index.get(registerKey(fc, start + i));
    if (value === undefined) return null;
    words.push(value);
  }
  return words;
}
export function selectAdjacent(previous: RegisterValue[], current: RegisterValue): RegisterValue[] {
  const key = registerKey(current.function, current.address);
  if (previous.some(r => registerKey(r.function, r.address) === key)) {
    return previous.filter(r => registerKey(r.function, r.address) !== key);
  }
  const neighbor = previous.find(r => r.function === current.function && Math.abs(r.address - current.address) === 1);
  return (neighbor ? [neighbor, current] : [current]).sort((a,b) => a.address - b.address);
}
export function newTagId(): string {
  return "tag:" + Array.from(globalThis.crypto.getRandomValues(new Uint8Array(16)), b => b.toString(16).padStart(2,"0")).join("");
}

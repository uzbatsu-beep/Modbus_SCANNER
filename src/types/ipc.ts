/**
 * Frontend IPC types.
 *
 * Types are generated from the Rust source by `ts-rs`:
 *   cd src-tauri && cargo run --bin gen_bindings
 * Do not edit `generated/` manually. Only UI constants live in this file.
 */

export * from "./generated/index";

/** Modbus function codes used by the UI. */
export const FC = {
  READ_COILS: 0x01,
  READ_DISCRETE_INPUTS: 0x02,
  READ_HOLDING_REGISTERS: 0x03,
  READ_INPUT_REGISTERS: 0x04,
  WRITE_SINGLE_COIL: 0x05,
  WRITE_SINGLE_REGISTER: 0x06,
  WRITE_MULTIPLE_COILS: 0x0f,
  WRITE_MULTIPLE_REGISTERS: 0x10,
  MASK_WRITE_REGISTER: 0x16,
  READ_WRITE_MULTIPLE_REGISTERS: 0x17,
} as const;

export const FC_NAMES: Record<number, string> = {
  0x01: "Read Coils (0x01)",
  0x02: "Read Discrete Inputs (0x02)",
  0x03: "Read Holding Registers (0x03)",
  0x04: "Read Input Registers (0x04)",
  0x05: "Write Single Coil (0x05)",
  0x06: "Write Single Register (0x06)",
  0x0f: "Write Multiple Coils (0x0F)",
  0x10: "Write Multiple Registers (0x10)",
  0x16: "Mask Write Register (0x16)",
  0x17: "Read/Write Multiple Registers (0x17)",
};

export const BAUD_RATES = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];

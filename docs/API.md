# IPC API reference

[Русская версия](API.ru.md)

The frontend talks to Rust via Tauri 2 `invoke` (commands) and `listen` (events).
All command errors arrive as human-readable `String`s. The typed wrappers live
in `src/api/mod.ts`; the payload types in `src/types/ipc.ts` (TypeScript mirror of
`src-tauri/src/scanner/types.rs`).

## Commands

### System

| Command | Args | Returns |
|---------|------|---------|
| `get_app_info` | — | `{name, version, rustc, debug}` |
| `get_log_level` | — | `string` (value of `RUST_LOG`) |

### Serial ports

**`list_serial_ports`** → `SerialPortInfo[]`

```ts
{ port_name: string;
  port_type: "usb" | "pci" | "bluetooth" | "unknown";
  vid?: number; pid?: number;
  manufacturer?: string; product?: string }
```

**`test_serial_port`** — args: `{ params: TestSerialParams }`

```ts
{ port_name: string; baud_rate: number;
  data_bits: 5|6|7|8; parity: "none"|"even"|"odd"; stop_bits: 1|2;
  mode: "rtu"|"ascii"; test_modbus: boolean }
```

Returns `{ opened, error?, modbus_responded, rtt_us?, response_hex? }`.
The test sends FC03 (unit 1, address 0, quantity 1) and waits 800 ms.

### Scanner lifecycle

| Command | Args | Returns | Effect |
|---------|------|---------|--------|
| `start_scan` | `{ target: ScanTarget, params: ScanParams }` | `void` | Spawns an engine task; returns immediately |
| `stop_scan` | — | `void` | Stops after the current request |
| `pause_scan` | — | `void` | Pauses between work items |
| `resume_scan` | — | `void` | Resumes |
| `get_scan_status` | — | `ScanStatus` | `"idle" \| "preparing" \| "running" \| "paused" \| "stopped" \| "finished" \| "failed"` |
| `get_scan_progress` | — | `ScanProgress` | Latest progress snapshot |
| `get_scan_results` | — | `ScanResults` | Full results snapshot |
| `clear_results` | — | `void` | Clears records and stats |
| `export_results` | `{ path: string, format: "csv"\|"json"\|"xml" }` | `number` | Writes file; returns device count |

`ScanTarget` variants:

```ts
// serial bus
{ type: "unit_id_range", from: number, to: number,
  port: { port_name, baud_rate, data_bits, parity, stop_bits },
  mode: "rtu" | "ascii" }

// IP range (from may be CIDR; to = null for single/CIDR)
{ type: "ip_range", from: string, to: string | null,
  port: number, kind: "tcp" | "udp" }

// one device + register plan
{ type: "single_device", target: DeviceTarget,
  register_plan: Array<{ function, start, quantity }> }
```

`ScanParams`:

```ts
{ timeout_ms: number; request_gap_ms: number; retries: number;
  probe_functions: number[]; stop_on_first: boolean;
  register_plan: Array<{ function, start, quantity }>;
  ip_concurrency: number }
```

### Manual Modbus commands

**`read_registers`** — `{ req: ReadRequest }`

```ts
{ target: ManualTarget, function: 1|2|3|4,
  start_address: number, quantity: number,
  timing?: { timeout_ms: number, retries?: number } }
```

**`write_registers`** — `{ req: WriteRequest }`, same shape with
`function: 5|6|16` and `values: number[]`.

**`send_raw_frame`** — `{ req: RawFrameRequest }`

```ts
{ target: ManualTarget, adu_hex: string,
  is_full_adu: boolean, timing?: ManualTiming }
```

`is_full_adu = false`: `adu_hex` is a PDU and the ADU is built automatically
(RTU: +unit+CRC; ASCII: with LRC; TCP: with MBAP). `true`: bytes are sent as is.

All three return `ManualResponse`:

```ts
{ ok: boolean; error?: string; exception_code?: number;
  values: number[]; response_hex?: string; rtt_us?: number }
```

`ManualTarget`:
`{ kind, endpoint, port?, unit_id, serial?, serial_mode? }` — for RTU/ASCII,
`endpoint` is the port name and `serial` must carry the port parameters.

### Configuration

| Command | Args | Returns |
|---------|------|---------|
| `get_config` | — | in-memory `Config` |
| `save_config` | `{ config: Config }` | `void` (also persists to `config.json`) |
| `load_config` | — | `Config` (reads from disk, or defaults) |

`Config` fields: `language`, `theme`, `scan_params`, `last_serial`,
`last_tcp_host`, `last_tcp_port`, `last_udp_host`, `last_udp_port`,
`export_dir`, `export_format`.

## Events

Subscribe with `listen` (wrapped by `onScanProgress` / `onScanEvent` in
`src/api/mod.ts`).

### `scan:progress` — `ScanProgress`

```ts
{ status: ScanStatus; total: number; done: number;
  found: number; current: string; errors: number;
  elapsed_ms: number }
```

Emitted after every work item. `total/done` are request counters.

### `scan:event` — `ScanEvent`

Discriminated union on `kind`:

```ts
{ kind: "status_changed", status: ScanStatus, message: string }
{ kind: "device_found", record: DeviceRecord }
{ kind: "device_updated", record: DeviceRecord }
{ kind: "probe_completed", device_key, function, result: ProbeResult } // reserved
{ kind: "finished", total, found, errors, elapsed_ms }
```

`DeviceRecord`:

```ts
{ key: string;                       // "rtu:COM3:1", "tcp:1.2.3.4:502:1"
  target: DeviceTarget;
  probes: Record<number, ProbeResult>;
  registers: Record<string, RegisterValue>;  // key "fc:address", e.g. "03:0000"
  first_seen_ms: number; last_seen_ms: number }
```

`ProbeResult` variants (`status` discriminator):

```ts
{ status: "ok", function, values: number[], raw_hex: string, rtt_us: number }
{ status: "exception", function, code: number, message: string, rtt_us: number }
{ status: "no_response", function }
{ status: "transport_error", function, message: string }
```

`RegisterValue`: `{ function, address, raw, as_hex, rtt_us }`.

## Frontend usage example

```ts
import { api, onScanEvent, onScanProgress } from "@/api/mod";

await onScanProgress((p) => console.log(p.done, "/", p.total));
await onScanEvent((e) => {
  if (e.kind === "device_found") console.log("found:", e.record.key);
});

await api.scanner.start(
  { type: "unit_id_range", from: 1, to: 247, mode: "rtu",
    port: { port_name: "COM3", baud_rate: 9600, data_bits: "eight",
            parity: "none", stop_bits: "one" } },
  { timeout_ms: 800, request_gap_ms: 20, retries: 1,
    probe_functions: [3, 4, 1, 2], stop_on_first: false,
    register_plan: [], ip_concurrency: 32 }
);
```

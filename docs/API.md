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
| `export_results` | `{ path: string, format: "csv"\|"json"\|"xml" }` | `number` | Writes file with the whole session; returns device count |
| `export_device` | `{ key: string, path: string, format: "csv"\|"json"\|"xml" }` | `boolean` | P2-08: exports only the selected `DeviceRecord` (probes, registers, FC43 identity); `false`/error if key is unknown |
| `save_project` | `{ path: string, name: string }` | `string` (path) | P3-04: writes devices + tag map + scan history to one JSON file; appends a snapshot (max 20) |
| `load_project` | `{ path: string }` | `Project` | P3-04: restores results and tags, syncs `config.json` |
| `start_monitor` | `{ watches: MonitorWatch[], interval_ms: number }` | `void` | P3-05: poll registers every 250–10000 ms |
| `stop_monitor` | — | `void` | Stop live polling |
| `get_monitor_state` | — | `MonitorValueState[]` | Current min/max/changed values and sample history |
| `get_traffic_log` | — | `TrafficRecord[]` | P3-06: last 2000 raw exchanges |
| `clear_traffic_log` | — | `void` | Clear the traffic log |
| `export_traffic_log` | `{ path: string, format: "csv"\|"json" }` | `number` | Export the log (count of records) |
| `set_loggers` | `{ configs: LoggerConfig[] }` | `LoggerStatus[]` | P3-07: install file loggers (CSV/TSV/JSONL, rotation none/daily/session/size) |
| `get_loggers` | — | `LoggerStatus[]` | Current logger status (written records, file, errors) |

`ScanTarget` variants:

```ts
// serial bus, known settings
{ type: "unit_id_range", from: number, to: number,
  port: { port_name, baud_rate, data_bits, parity, stop_bits },
  mode: "rtu" | "ascii" }

// serial bus, unknown settings (P2-01): two-phase sweep
{ type: "serial_auto",
  sweep: {
    port_name: string, mode: "rtu" | "ascii",
    baud_rates: number[],                 // priority order
    framings: Array<{ data_bits, parity, stop_bits }>, // priority order
    discovery_unit_ids: number[],         // few ids used in phase 1
    unit_from: number, unit_to: number    // full range for phase 2
  } }

// adaptive register/coil map on one device (P2-03)
{ type: "register_scan",
  scan: {
    target: ManualTarget,
    function: 1|2|3|4,
    start: number, end: number,       // inclusive addresses
    mode: "quick" | "balanced" | "deep",
    batch: number                      // 0 = mode default
  } }

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
  ip_concurrency: number; identify_devices?: boolean;
  tcp_framing?: "mbap"|"rtu"; udp_mbap?: boolean }
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

**`inspect_registers`** — `{ values: number[] }` → `DecodeResult`: all
interpretations (INT16/UINT16/INT32/UINT32/FLOAT32/ASCII × ABCD/BADC/CDAB/DCBA)
plus labelled float hypotheses. **`decode_register`** —
`{ values: number[], spec: DecodeSpec }` → `DecodedValue | null` (used by named
tags; `DecodeSpec` = `{ data_type, byte_order, multiplier?, divider?, offset? }`).

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
`export_dir`, `export_format`, `tags` (named register tags, P3-03).

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
{ kind: "ip_target_classified", status: IpTargetStatus }
{ kind: "probe_completed", device_key, function, result: ProbeResult }
{ kind: "finished", total, found, errors, elapsed_ms }
```

`probe_completed` is emitted once per finished discovery probe (including
timeouts and transport errors) and the frontend writes one line to the event log.

`DeviceRecord`:

```ts
{ key: string;                       // "rtu:COM3:1", "tcp:1.2.3.4:502:1"
  target: DeviceTarget;
  probes: Record<number, ProbeResult>;
  registers: Record<string, RegisterValue>;  // key "fc:address", e.g. "03:0000"
  device_identification?: DeviceIdentification | null;
  first_seen_ms: number; last_seen_ms: number }

// FC43/MEI14, P2-05
interface DeviceIdentification {
  read_code: number; conformity_level: number;
  more_follows: boolean; next_object_id: number;
  objects: Array<{ id: number; value: string }>
}
```

`ProbeResult` variants (`status` discriminator):

```ts
{ status: "ok", function, values: number[], raw_hex: string, rtt_us: number }
{ status: "exception", function, code: number, message: string, rtt_us: number }
{ status: "no_response", function }
{ status: "transport_error", function, message: string }
```

`RegisterValue`: `{ function, address, raw, as_hex, rtt_us }`.

`Project` (P3-04): `{ name, version, created_ms, updated_ms,
results: ScanResults, tags: TagDefinition[], history: ScanSnapshot[] }`, where
`ScanSnapshot = { timestamp_ms, label, results }`.

### `monitor:update` — `MonitorUpdate` (P3-05)

`{ values: MonitorValueState[], cycle_ms: number }` — emitted once per monitor
poll cycle. `MonitorValueState` carries `key`, `name`, `unit`, latest `values`,
`min`/`max`, a `changed` flag and up to 5000 `samples` (one hour window).

`LoggerConfig` (P3-07): `{ id, path, format: "csv"|"tsv"|"json",
rotation: "none"|"daily"|"session"|{ size_mb: number }, keys?: string[],
enabled?: boolean }`. Loggers consume monitor samples each cycle; `keys = []`
means all watches. Daily/session rotation adds a date/timestamp suffix, size
rotation starts a timestamped file when the limit is exceeded.

### `traffic:record` — `TrafficRecord` (P3-06)

`{ id, timestamp_ms, kind, endpoint, unit_id, function?, tx_hex, rx_hex?, rtt_us,
status }` — one raw exchange per probe/read, including failed ones. The UI keeps
the last 2000 records and can filter by function/status and export CSV/JSON.

### IP target classification (P2-04)

`ScanResults.ip_targets: IpTargetStatus[]` — one entry per scanned `ip:port`:

```ts
{ endpoint: string; port: number;
  class: "valid_modbus" | "port_open_no_modbus" | "connection_refused"
       | "timeout" | "unreachable" | "unknown";
  unit_ids_probed: number[];
  responses_ok: number; responses_exception: number;
  timeouts: number; transport_errors: number;
  last_seen_ms: number }
```

The class never downgrades: a positive answer beats an open port, which beats
refused / timeout / unreachable. An open TCP socket with no valid Modbus answer
is reported as `port_open_no_modbus`, not as a device.

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

# IPC API reference

[Русская версия](API.ru.md)

The frontend talks to Rust via Tauri 2 `invoke` (commands) and `listen` (events).
All command errors arrive as human-readable `String`s. The typed wrappers live
in `src/api/mod.ts`; the payload types in `src/types/ipc.ts` (TypeScript mirror of
`src-tauri/src/scanner/types.rs`).

## Local REST + WebSocket API (E-03)

Started with `modbus-scanner serve --port 8787` or from the desktop app via the
`start_api_server` Tauri command. Binds to **127.0.0.1 only** and **requires authentication** for every
HTTP route (including `/health`) and WebSocket upgrade. All scan endpoints
reuse the same `ScannerHandle` as GUI and CLI, including the OT IP policy.

**Bootstrap:** set a unique `MODBUS_SCANNER_API_TOKEN` in the process
environment **before** starting the application or `serve` command.
It must have at least 24 printable ASCII characters; otherwise the API refuses
to start. Send `Authorization: Bearer <TOKEN>` on every request. Do not place
tokens in URLs, query strings, or Git repositories. Browser requests with
an `Origin` other than `http://127.0.0.1:<port>` or
`http://localhost:<port>` are forbidden. Standard browser WebSocket clients
cannot set arbitrary Authorization headers; use a native client for authenticated
WS subscriptions. Binding to loopback alone is **not** an access-control policy.

**OT scan safety:** `ScanParams.ip_allowlist` is a list of literal IP/CIDRs
(e.g. `["192.168.1.0/24"]`), required for all TCP/UDP scan targets.
An empty list denies all IP scans before network I/O. `ip_exclusions`
overrides allowlist entries (e.g. `["192.168.1.254"]`).
DNS names are prohibited to prevent authorization bypass/rebinding;
all non-excluded requested IPs must fall within the allowed scope.
`quiet_mode: true` limits outbound scanner transmissions to
**2 req/sec**, limits TCP connection attempts to **2/sec**, and forces
IP concurrency to 1. Retries, FC43, and register reads all consume slots.
It does not govern the separate manual/monitor subsystems, and it is not
an emergency stop.

| Method | Path | Result |
|--------|------|--------|
| GET | `/health` | `{status:"ok", service, version, timestamp_ms}` |
| GET | `/api/v1/status` | `{status, progress}` |
| GET | `/api/v1/results` | `ScanResults` |
| GET | `/api/v1/traffic` | last 2000 `TrafficRecord`s |
| POST | `/api/v1/scan` | body `{target: ScanTarget, params: ScanParams}` → `{status:"started"}` |
| POST | `/api/v1/scan/stop` | request immediate cancellation → `{"status":"stopping"}` (poll `/status` for terminal `stopped`) |
| POST | `/api/v1/scan/pause` | pause |
| POST | `/api/v1/scan/resume` | resume |
| WS | `/api/v1/events` | JSON messages `{event, payload}` (scan:event, scan:progress, monitor:update, traffic:record) |

Tauri commands for the in-app server: `start_api_server(port)`, `stop_api_server`,
`get_api_status() -> port | null`.

## Commands

### System

| Command | Args | Returns |
|---------|------|---------|
| `get_app_info` | — | `{name, version, rustc, debug}` |
| `get_log_level` | — | `string` (value of `RUST_LOG`) |
| `start_api_server` | `port: number` | `void` (starts the local REST/WS server) |
| `stop_api_server` | — | `void` |
| `get_api_status` | — | `number \| null` (listening port) |

### Device profiles (E-04)

| Command | Args | Returns |
|---------|------|---------|
| `get_profiles` | — | `DeviceProfile[]` |
| `save_profiles` | `{ profiles: DeviceProfile[] }` | `DeviceProfile[]` |
| `import_profiles` | `{ path: string, format: "csv"\|"json" }` | `DeviceProfile[]` (merged into config) |
| `create_profile_from_device` | `{ deviceKey: string }` | `DeviceProfile` (skeleton from FC43 identity) |
| `suggest_profile` | `{ deviceKey: string }` | `DeviceProfile \| null` (matched by vendor/product/revision) |
| `apply_profile` | `{ deviceKey: string, profileId: string }` | `TagDefinition[]` (replaces tags for that device) |

`DeviceProfile = { id, vendor, product_code, revision, name, description?,
registers: ProfileRegister[] }`; `ProfileRegister = { function, address, name,
spec: DecodeSpec, unit?, description? }`. Identity matching is case-insensitive
and an empty profile field acts as a wildcard. A sample CSV is in
`examples/profiles.csv`.

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
| `stop_scan` | — | `void` | Requests cancellation immediately; Stopped only after cleanup |
| `pause_scan` | — | `void` | Pauses between work items |
| `resume_scan` | — | `void` | Resumes |
| `get_scan_status` | — | `ScanStatus` | `"idle" \| "preparing" \| "running" \| "paused" \| "stopping" \| "stopped" \| "finished" \| "failed"` |
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
| `compare_scans` | `{ left: ScanResults, right: ScanResults, left_label, right_label }` | `ScanDiff` | P3-08: diff register maps of two snapshots |

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
  tcp_framing?: "mbap"|"rtu"; udp_mbap?: boolean;
  tls?: { client_cert?: string|null; client_key?: string|null } | null }
```

### Manual Modbus commands

**`read_registers`** — `{ req: ReadRequest }`

```ts
{ target: ManualTarget, function: 1|2|3|4,
  start_address: number, quantity: number,
  timing?: { timeout_ms: number, retries?: number } }
```

**`write_registers`** — `{ req: WriteRequest }`, same shape with
`function: 5|6|16` and `values: number[]`, plus explicit `confirm_write: true`.

**Manual I/O safety (PR #6):** `read_registers` may retry up to 5 times
according to `timing.retries`. Both `write_registers` and `send_raw_frame`
are strictly **single-shot**, regardless of the supplied retry count. The
backend checks positive write acknowledgments: FC05/FC06 echo the exact address
and value; FC10 echoes the exact start address and quantity. A mismatching
acknowledgment is a failure, never `ok: true`. Broadcast unit 0 is blocked
on every transport for write/raw operations.

If a write or raw request times out, the outcome is **unknown**: the device
may have applied the operation even though the response was lost. No implicit
retransmission takes place. Verify the relevant device state before deciding
whether to issue a new write. Raw requests remain explicitly unsafe and require
`confirm_unsafe: true` for each send. Raw-frame `ok: true` means only that
response bytes arrived: it is **not** a validated write acknowledgment.
Empty payloads and payloads over the Modbus size limit are rejected.

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

`ScanDiff` (P3-08): `{ left_label, right_label, left_devices, right_devices,
added_devices, removed_devices, devices: DeviceDiff[], registers: RegisterDiff[],
added, removed, changed }`. Each `RegisterDiff` has `device_key`, `function`,
`address`, `change: "added"|"removed"|"changed"|"unchanged"`, `old_value`,
`new_value`. Only changed registers are listed.

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

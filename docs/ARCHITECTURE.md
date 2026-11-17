# Architecture

[Русская версия](ARCHITECTURE.ru.md)

## Big picture

```
┌────────────────────────────── Vue 3 frontend ──────────────────────────────┐
│ Views (Scan / Results / Manual / Settings)                                 │
│   └─ Pinia stores (scanner, devices, serial, settings)                     │
│        └─ src/api/mod.ts  ── invoke() / listen() ──────────────┐           │
└────────────────────────────────────────────────────────────────┼───────────┘
                                  Tauri IPC (JSON)              │
┌────────────────────────────────────────────────────────────────┼───────────┐
│ Rust backend (src-tauri)                                       ▼           │
│  ┌────────────────┐  ┌────────────────┐  ┌─────────────────────────────┐  │
│  │ commands/      │  │ state.rs       │  │ scanner/                    │  │
│  │ #[tauri::cmd]  │──▶ AppState       │◀─│ ScannerHandle (tokio task)  │  │
│  └───────┬────────┘  │ config Mutex   │  │ strategies / types / engine │  │
│          │           └────────────────┘  └──────────┬──────────────────┘  │
│          │                                          │ emits               │
│          ▼                                          ▼ scan:event/progress │
│  ┌────────────────┐  ┌────────────────┐                                    │
│  │ modbus/        │◀─│ transport/     │  serial (RTU/ASCII), TCP, UDP      │
│  │ PDU/ADU/codecs │  │ async I/O      │                                    │
│  └────────────────┘  └────────────────┘                                    │
└────────────────────────────────────────────────────────────────────────────┘
```

## Layers

### 1. Protocol layer — `src-tauri/src/modbus/`

Pure protocol code, no I/O:

| Module | Responsibility |
|--------|----------------|
| `common.rs` | `UnitId`, `Address`, `Quantity`, `TransportKind`, `SerialMode`, `ModbusError` with all standard exception codes |
| `frame.rs` | `FunctionCode`, `Pdu` (Request/Response/Exception), `Adu` variants, read/write PDU builders and response parsers |
| `rtu.rs` | RTU ADU: `encode` / `decode` with Modbus CRC-16 (poly `0xA001`, init `0xFFFF`, low byte first) |
| `ascii.rs` | ASCII ADU: `':' + hex(payload + LRC) + CRLF`, 8-bit LRC (two's complement of byte sum) |

TCP MBAP framing currently lives in the transport layer (`transport/tcp.rs`) and
in the ADU builders of the scanner engine (`build_adu`). See ROADMAP for the
planned move into `modbus/tcp.rs`.

### 2. Transport layer — `src-tauri/src/transport/`

A single async trait `Transport` (via `async-trait`):

```rust
#[async_trait]
pub trait Transport: Send {
    fn name(&self) -> String;
    async fn reset(&mut self) -> TransportResult<()>;      // flush stale RX
    async fn close(&mut self) -> TransportResult<()>;
    async fn send_raw(&mut self, adu: &[u8]) -> TransportResult<()>;
    async fn recv_raw(&mut self, hint: usize) -> TransportResult<Vec<u8>>;
    async fn transact(&mut self, adu, timeout, hint) -> TransportResult<Vec<u8>>;
}
```

Implementations:

- `SerialTransport` — `tokio-serial`, configurable baud/parity/data/stop bits,
  RX pre-flush, t3.5-style inter-frame gap. RTU uses a silence-based frame end
  (5 ms), ASCII reads until CRLF.
- `TcpTransport` — `tokio::net::TcpStream`, `TCP_NODELAY`, MBAP length framing,
  transaction-id generation. Also accepts prepackaged MBAP frames as-is
  ("RTU over TCP" / gateway scenarios).
- `UdpTransport` — `tokio::net::UdpSocket`, one datagram = one ADU, no stream
  framing; absence of a datagram is reported as `Timeout`.

### 3. Scanner layer — `src-tauri/src/scanner/`

- `types.rs` — the **IPC contract**: `ScanTarget`, `ScanParams`, `ProbeResult`,
  `DeviceRecord`, `RegisterValue`, `ScanStatus`, `ScanProgress`, `ScanEvent`,
  `ScanResults`/`ScanStats`. All types derive `serde::Serialize/Deserialize`;
  `src/types/ipc.ts` is their exact TypeScript mirror.
- `strategies.rs` — `ScanStrategy` trait (`Send`) producing `WorkItem`s:
  - `UnitIdStrategy`: unit-id range on one serial port, `probe_functions` per id;
  - `IpRangeStrategy`: IPv4 range / CIDR, unit ids 1 and 255 by default;
  - register plan → extra `RegisterRead` work after a device is found.
- `engine.rs` — `ScannerHandle` (stored in Tauri state) and the tokio task:
  - control via `mpsc<ControlMsg>` (Pause/Resume/Stop) + `watch<bool>` for pause;
  - status via `watch<ScanStatus>`;
  - results in `Arc<RwLock<ScanResults>>` (parking_lot);
  - one serial transport reused for the whole bus scan (strictly sequential);
  - TCP/UDP transports opened per request;
  - events emitted to the frontend: `scan:progress`, `scan:event`.

### 4. Command layer — `src-tauri/src/commands/`

Thin `#[tauri::command]` functions returning `Result<T, String>`. They translate
IPC calls into scanner/transport operations. Files: `system`, `serial`,
`scanner`, `modbus_cmd`, `config`. All commands are registered in `lib.rs`
via `tauri::generate_handler!`.

### 5. Frontend — `src/`

- `views/` — four tabs: Scan, Results, Manual, Settings.
- `stores/` — Pinia: scanner state/log, devices, serial ports, settings.
- `api/mod.ts` — typed wrappers around `@tauri-apps/api/core` `invoke` and
  `@tauri-apps/api/event` `listen`.
- `types/ipc.ts` — mirrors Rust types; keep in sync manually (a schema
  generator is on the ROADMAP).

## Async and concurrency model

- Tauri manages app state (`AppState`: `ScannerHandle` + `Config` in a mutex).
- `start_scan` spawns one tokio task; the IPC call returns immediately.
- Serial scans **must** be sequential — a shared bus cannot multiplex requests.
- IP scans are currently also executed sequentially by the engine; the
  `ip_concurrency` parameter is reserved for the planned bounded-concurrent
  worker pool (with a separate transport per target).
- Write guards (`parking_lot::RwLockWriteGuard`) are deliberately confined to
  scopes that contain no `.await` — otherwise the spawned future would not be
  `Send`. See `engine.rs` `execute_probe` for the pattern.
- The UI receives progress/results via events instead of polling.

## Data flow of a single scan

1. Vue builds a `ScanTarget` + `ScanParams`, calls `start_scan`.
2. `ScannerHandle::start` builds a strategy, spawns the engine task, sets status.
3. The engine loops: `strategy.next_item()` → build ADU → transport transact →
   verify unit id / function / checksum → classify response.
4. On a response, a `DeviceRecord` is created/updated in the shared results and
   a `device_found` / `device_updated` event is emitted.
5. If a register plan exists, extra `RegisterRead` items are executed and merged
   into `record.registers`.
6. Progress events keep the status bar updated; `get_scan_results` returns the
   full snapshot for the results table.

## Configuration persistence

`Config` lives in `state.rs`; `get_config`/`save_config`/`load_config` read/write
`config.json` under Tauri's `app_config_dir()` (e.g.
`%APPDATA%\com.modbus-scanner.app\config.json` on Windows).

## Build boundaries

- `src-tauri/tauri.conf.json` — frontend dist `../dist`, window, bundle, icons.
- `src-tauri/capabilities/default.json` — Tauri 2 capability set (core:default,
  shell:allow-open).
- `src-tauri/build.rs` — `tauri_build::build()` (generates resource/script).

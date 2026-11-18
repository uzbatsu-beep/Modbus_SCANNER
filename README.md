# Modbus Scanner

[![CI](https://github.com/uzbatsu-beep/Modbus_SCANNER/actions/workflows/ci.yml/badge.svg)](https://github.com/uzbatsu-beep/Modbus_SCANNER/actions/workflows/ci.yml)

[Русская версия](README.ru.md)

Cross-platform desktop tool for discovering and probing **Modbus** devices.
Rust backend (Tauri 2) + Vue 3 / TypeScript frontend. The Modbus protocol stack
is implemented in-house — no third-party Modbus crate — so framing, timeouts and
scan behaviour stay under our full control.

## Features

| Transport | What it scans |
|-----------|---------------|
| **Modbus RTU** | Slave IDs 0–247 on a serial port, all common baud rates (1200…115200), bit formats 8N1/8E1/8O1/7N1 etc. |
| **Modbus ASCII** | Slave IDs 0–247, hex framing with LRC, `:` + CRLF delimiters. |
| **Modbus TCP** | IP ranges or CIDR subnets, port 502 by default, Unit IDs 1/255 (gateway routing). |
| **Modbus UDP** | IP ranges, connectionless probe (no reply = no device), RTU-style ADU. |

**Function codes:** FC01 Read Coils, FC02 Read Discrete Inputs, FC03 Read Holding
Registers, FC04 Read Input Registers, FC05/06/0F/10/16/17 writes (in the manual
console and register map).

**Instrumentation:**
- Device discovery with configurable response timeout, retries and probe function codes
- Live progress and found-device events over Tauri events
- Register map reads for discovered devices (hex / dec / binary views)
- Manual request console: read, write, raw frame send
- Serial port enumeration (VID/PID, manufacturer, product) and a port test tool
- Result export to CSV / JSON / XML
- Configuration persistence (`config.json` in the Tauri app config dir)

## Project layout

```
.
├── src/                       # Vue 3 + TypeScript frontend
│   ├── api/                   # Typed Tauri invoke/event wrappers
│   ├── components/            # layout / scan forms / results / common
│   ├── stores/                # Pinia: scanner, devices, serial, settings
│   ├── types/ipc.ts           # TypeScript mirror of the Rust IPC contract
│   └── views/                 # Scan, Results, Manual, Settings
├── src-tauri/                 # Rust backend (Tauri 2)
│   └── src/
│       ├── modbus/            # Protocol layer: PDU/ADU, RTU (CRC-16), ASCII (LRC)
│       ├── transport/         # Async I/O: serial (tokio-serial), TCP, UDP
│       ├── scanner/           # Engine, strategies, scan types/results
│       ├── commands/          # #[tauri::command] handlers
│       └── state.rs           # Managed app state + configuration
├── docs/                      # Architecture, protocol, scanning, API, development
├── package.json               # Frontend dependencies and scripts
└── README.md / README.ru.md
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — layers, async model, data flow
- [Modbus protocol](docs/MODBUS.md) — framing, function codes, exceptions, timing
- [Scanning](docs/SCANNING.md) — strategies, timeouts, safety
- [IPC API](docs/API.md) — commands and events with payload schemas
- [Development](docs/DEVELOPMENT.md) — setup, build, tests, conventions
- [Research](docs/RESEARCH.md) — market/community/academic findings and sources
- [Development plan](docs/PLAN.md) — prioritized phases v0.2 → v1.0
- [Roadmap](docs/ROADMAP.md) — current status and next steps
- [Security](docs/SECURITY.md) — safety of writing to unknown devices

## Quick start

Requirements: **Node.js 18+**, **Rust 1.77+**, **Tauri 2 prerequisites**
([official guide](https://v2.tauri.app/start/prerequisites/)). On Windows, the
WebView2 runtime is required.

```bash
npm install
npm run tauri dev        # starts Vite + the Tauri window
```

Production build:

```bash
npm run tauri build      # installers in src-tauri/target/release/bundle/
```

Useful commands:

```bash
npm run typecheck        # vue-tsc
cargo check              # in src-tauri/
cargo test --lib         # protocol unit tests
```

## Status

This is an early MVP: the protocol layer, transports, scanner engine and UI are
implemented and the project compiles. Unreleased work covers v0.2 (field
discovery, adaptive scan, IP classification, FC43, exports) and v0.3 (decoder,
projects, live monitor, traffic log, loggers, scan diff, RU/EN UI and a light
theme) — see [ROADMAP](docs/ROADMAP.md) and [PLAN](docs/PLAN.md).

## License

[GNU GPL v3.0](LICENSE) — free software, copyleft. See [LICENSE](LICENSE) for the
full text.

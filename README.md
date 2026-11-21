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
- [v2.0 Platform research](docs/RESEARCH_V2.md) — gateways, web companion, IEC 62443, plugins
- [Development plan](docs/PLAN.md) - prioritized phases v0.2 → v1.0
- [Audit](docs/AUDIT.md) - git, reliability, documentation and transparency findings
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

### Headless CLI

The same engine also runs without the GUI. `scan` prints a JSON report (or writes
it with `--output`):

```bash
# serial bus, auto baud/parity sweep, report to file
src-tauri/target/release/modbus-scanner.exe scan --port COM3 --auto --output report.json

# Modbus TCP subnet
src-tauri/target/release/modbus-scanner.exe scan --ip 192.168.1.0/24 --allow-ip 192.168.1.0/24 --quiet-mode --tcp-port 502

# raw RTU over TCP, UDP, custom timeouts
src-tauri/target/release/modbus-scanner.exe scan --ip 10.0.0.5 --allow-ip 10.0.0.5 --rtu-over-tcp --timeout-ms 300
src-tauri/target/release/modbus-scanner.exe scan --ip 10.0.0.5 --allow-ip 10.0.0.5 --udp --udp-mbap

# Modbus Security (TLS port 802, optional mutual TLS)
src-tauri/target/release/modbus-scanner.exe scan --ip 10.0.0.5 --allow-ip 10.0.0.5 --tls --tls-cert client.pem --tls-key client.key
```

Run `modbus-scanner scan --help` for all options. With no subcommand the desktop
app starts. The CLI uses an `EventSink` instead of Tauri, so GUI and CLI share
exactly the same scanning logic.

A local REST + WebSocket API is available (loopback only):

```bash
# Set MODBUS_SCANNER_API_TOKEN in the process environment before starting.
# Use a unique secret, at least 24 printable ASCII characters (do not commit).
modbus-scanner.exe serve --port 8787
curl -H "Authorization: Bearer $MODBUS_SCANNER_API_TOKEN" http://127.0.0.1:8787/health
curl -H "Authorization: Bearer $MODBUS_SCANNER_API_TOKEN" http://127.0.0.1:8787/api/v1/results
```

All network scans require explicit allowed IP/CIDR scopes (`--allow-ip`).
Add `--exclude-ip` for forbidden addresses and `--quiet-mode` for a global
2 requests/second scanner transmission cap (including retries and FC43).
The API refuses to start without its Bearer token; see
[docs/API.md](docs/API.md) for authenticated REST and WebSocket use.

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

**Release candidate:** v1.0.1 protocol and OT safety changes are in main.
[Windows QA](docs/WINDOWS_QA.md) builds test EXE/MSI without a Release.
Official publishing is now **manual and approval-gated**; configure
[Repository rulesets and release approval](docs/RELEASE_GATES.md) before release.


The protocol layer, transports, scanner engine and UI are implemented, and the
project is released as **v1.0.0** (headless CLI, local REST/WS API, device
profiles, Modbus TLS, automated releases, generated IPC types, mock tests and
3-OS CI). Earlier releases `v0.1.0`/`v0.1.1` covered the MVP and the v0.2/v0.3
field-discovery and decode/monitor feature sets — see [ROADMAP](docs/ROADMAP.md)
and [PLAN](docs/PLAN.md).

## License

[GNU GPL v3.0](LICENSE) — free software, copyleft. See [LICENSE](LICENSE) for the
full text.

## Release vs development status (2026-11-21)

- **Published:** [v1.0.0](https://github.com/uzbatsu-beep/Modbus_SCANNER/releases/tag/v1.0.0). The checked-in `1.0.1` manifests describe an **unreleased** candidate, not a downloadable approved v1.0.1.
- **Automation:** tests and Windows QA verify code and installer generation, **not physical RS-485 operation**. Follow [field acceptance](docs/FIELD_VALIDATION_v1.0.1.md) before any new production release.
- **Governance:** the live `main` Ruleset currently requires one independent review, while [PR #10](https://github.com/uzbatsu-beep/Modbus_SCANNER/pull/10) proposes a zero-review solo-maintainer policy. The JSON template alone does not change GitHub Settings.
- For documentation by topic, risks and evidence, see [documentation index](docs/INDEX.md), [current audit](docs/AUDIT_2026-11-21.md) and [manual release gates](docs/RELEASE_GATES.md).

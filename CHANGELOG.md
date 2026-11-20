# Changelog / История изменений

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Все значимые изменения проекта фиксируются здесь. Формат основан на
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), версионирование —
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Build / Release Candidate (PR #3)
- Separate Windows QA workflow builds NSIS EXE + MSI from the PR source without releasing or tagging; includes SHA256 checksums, commit metadata and artifact retention.
- Consistent version validation across npm, Cargo, Tauri and optional release tag; official release workflow runs Rust regression tests before packaging.

### Added
- RESEARCH_V2 §9: live registry data (crates.io/GitHub) on OPC UA, MQTT, BACnet and WASM libraries, GPL-3.0 licence compatibility and decisions for G-01…G-06

### Changed
- ROADMAP/AUDIT refreshed for the published v1.0.0; v1.1/v1.2 exit criteria and the v2.0 technology choices/order made explicit

### Fixed
- Audit: progress counters no longer double-count timeouts; stale architecture/protocol docs, broken `lint` script and misleading Cargo comment corrected

### Added — PR #2 (field validation)
- Deterministic loopback Modbus TCP fixture for full scanner discovery, register reads,
  split MBAP payloads, exceptions, wrong transaction IDs and silent endpoints.
- Cross-platform CI protocol-smoke job; no external hardware or network required.

### Fixed — PR #2
- Rejected empty/unsafe discovery FC configurations before any network access.
- Normal Modbus read replies are explicitly tagged as PDU Response (not a generic Request), restoring decoded register data in scanner and manual reads.
- Scan status uses watch send_replace so CLI/API callers receive terminal status
  even if no watch receiver was active when the scan began.

## [1.0.1] - release candidate (PR #1)

### Fixed
- Fixed TCP MBAP framing: Length includes the Unit ID byte already consumed in the 7-byte header.
- Serial RTU waits for the first byte until the configured response timeout and uses baud-correct t3.5; ASCII waits for CRLF.
- Bounds CIDR allocations to /16 or narrower and handles /0 safely.
- Single-device scans now accept TCP/UDP targets, preserving serial RTU/ASCII modes.
- Validates register read/write counts, serial unit IDs and 16-bit address boundaries; fixes overflow at register 65535.
- Serial port test validates Modbus response CRC/LRC, address and FC, and reports measured RTT.

### Safety
- Manual writes and raw frames now require a UI confirmation and a per-request opt-in enforced by Rust.
- Serial broadcast writes (unit 0) are blocked; automated discovery remains read-only.

## [1.0.0] - 2026-11-19

### Added
- E-08: in-memory MockTransport engine tests and CI matrix extended to macOS (v1.0 complete)
- E-07: generated TypeScript IPC bindings from Rust via `ts-rs` (`npm run gen:types`, 67 files, facade, CI freshness check)
- E-06: tag-triggered GitHub Actions release workflow (MSI/NSIS build, artifact upload, GH Release with assets)
- E-05: Modbus Security TLS (port 802) with webpki root validation and optional mutual TLS (`--tls`, `ScanParams.tls`)
- E-04: reusable device profiles (CSV/JSON import, FC43 identity matching, one-click tag setup, Profiles tab)
- E-03: local REST + WebSocket API (loopback only) with `/health`, scan control/results/traffic endpoints, WS event stream, `serve` CLI subcommand and Tauri lifecycle commands
- E-02: headless `scan` CLI with JSON reports, serial auto-sweep and TCP/UDP targets (same engine as GUI)
- E-01: framework-agnostic scanner core (`EventSink`, `TrafficSink`, `SampleSink`); Tauri is isolated in `bridge.rs`, enabling a future CLI/headless mode

### Docs
- Added v2.0 platform research (gateways, Sparkplug B/OPC UA/BACnet, IEC 62443 policies, plugin architecture) and a prioritised G-01…G-06 backlog
- Refreshed the development roadmap with v1.0–v2.0 milestones, scoring, KPIs and risks

## [0.1.1] - 2026-11-19

### Fixed
- Removed the last unused Rust enum so release builds are warning-free
- Register-plan reads after a serial probe now reuse the shared serial port instead of an unopened one
- Vite `@` alias (frontend build failed in CI with "Rollup failed to resolve @/App.vue")
- CI backend jobs now create a `dist/` placeholder before `cargo check` (generate_context!)
- CI Node.js version bumped to 22

### Added
- P3-09: built-in RU/EN localization for all UI panels and a light theme (v0.3 complete)
- P3-08: scan diff (compare current session with project snapshots, register-level added/removed/changed)
- P3-07: multiple file loggers for monitor samples (CSV/TSV/JSONL, rotation none/daily/session/size)
- P3-06: raw TX/RX traffic log (`traffic:record` event, filters, CSV/JSON export, dedicated tab)
- P3-05: live register monitor (250 ms–10 s), min/max, change highlighting and SVG trends with `monitor:update` event
- P3-04: portable projects (`save_project`/`load_project`) with devices, tags and up to 20 scan snapshots
- P3-01/02/03: register decoder (INT/UINT/FLOAT32/ASCII, ABCD/BADC/CDAB/DCBA), labelled float hypotheses and named tags with scaling persisted in config
- `probe_completed` events for every finished probe (including timeouts/errors) and per-probe log lines in the UI — v0.2 complete
- MBAP transaction-id validation; TCP ADU selector (`mbap` / `rtu` over TCP) and UDP MBAP mode
- P2-08: `export_device` command — export only the selected device to CSV/JSON/XML (probes, register map, FC43 identity)
- P2-05: best-effort FC43/MEI14 device identification (VendorName, ProductCode, Revision) with a device-detail view and `identify_devices` flag
- P2-04: IP target classification (valid Modbus / port open / refused / timeout / unreachable) with a results-table view and `ip_target_classified` event
- P2-03: adaptive register/coil map (`register_scan`) with Quick/Balanced/Deep strategies, exception-aware block splitting and a three-timeout guard
- P2-01: two-phase automatic serial-settings sweep (`serial_auto` target) with early exit on the first valid Modbus frame
- P2-02: `ScanParams` are now effective — per-request timeout, retries, inter-request gap and bounded IP concurrency (serial stays sequential)
- Market/community/academic research report and prioritized development plan (EN/RU: docs/RESEARCH.md, docs/RESEARCH.ru.md, docs/PLAN.md, docs/PLAN.ru.md)
- GNU GPL v3.0 license, repository metadata, CI badge, issue/PR templates

## [0.1.0] - 2026-11-17

### Added
- Tauri 2 (Rust) backend and Vue 3 + TypeScript frontend structure
- In-house Modbus protocol layer: RTU (CRC-16), ASCII (LRC), TCP (MBAP), UDP
- Async transports: serial (`tokio-serial`), TCP, UDP
- Scanning engine with unit-id/IP-range strategies, pause/resume/stop and events
- UI: Scan, Results, Manual, Settings tabs; device table and register map
- Manual console: read / write / raw frame
- Serial port enumeration and port test
- Export to CSV / JSON / XML, config persistence
- Bilingual documentation (EN/RU): README, architecture, protocol, scanning, API, development
- GitHub Actions CI: frontend typecheck/build, backend check/tests on Windows and Linux

[Unreleased]: https://github.com/uzbatsu-beep/Modbus_SCANNER/compare/v1.0.0...HEAD
[1.0.1]: https://github.com/uzbatsu-beep/Modbus_SCANNER/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/uzbatsu-beep/Modbus_SCANNER/releases/tag/v1.0.0
[0.1.1]: https://github.com/uzbatsu-beep/Modbus_SCANNER/releases/tag/v0.1.1
[0.1.0]: https://github.com/uzbatsu-beep/Modbus_SCANNER/releases/tag/v0.1.0

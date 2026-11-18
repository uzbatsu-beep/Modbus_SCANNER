# Changelog / История изменений

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Все значимые изменения проекта фиксируются здесь. Формат основан на
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), версионирование —
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-11-19

### Fixed
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

[Unreleased]: https://github.com/uzbatsu-beep/Modbus_SCANNER/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/uzbatsu-beep/Modbus_SCANNER/releases/tag/v0.1.1
[0.1.0]: https://github.com/uzbatsu-beep/Modbus_SCANNER/releases/tag/v0.1.0

# Changelog / История изменений

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Все значимые изменения проекта фиксируются здесь. Формат основан на
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), версионирование —
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- Register-plan reads after a serial probe now reuse the shared serial port instead of an unopened one
- Vite `@` alias (frontend build failed in CI with "Rollup failed to resolve @/App.vue")
- CI backend jobs now create a `dist/` placeholder before `cargo check` (generate_context!)
- CI Node.js version bumped to 22

### Added
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

[Unreleased]: https://github.com/uzbatsu-beep/Modbus_SCANNER/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/uzbatsu-beep/Modbus_SCANNER/releases/tag/v0.1.0

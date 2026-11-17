# Changelog / История изменений

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Все значимые изменения проекта фиксируются здесь. Формат основан на
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), версионирование —
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
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

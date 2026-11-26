# Changelog / История изменений

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Все значимые изменения проекта фиксируются здесь. Формат основан на
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), версионирование —
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Typed monitoring (PR #14)
- Canonical typed decoding, signed trends and true time axis with quality gaps; last-good indication and session statistics.
- Bounded 10,000/channel default ring and 200 ms incremental UI batches; Logger v2 asynchronous bounded queue and distinct output files.
- Reject invalid scaling and preserve RAW on decode errors; no fabricated RTT or cycle time. No new protocol scheduler or release.


### Register identity (PR #13)
- Introduce AddressSpace, RegisterKey, RegisterSpan and independent tag IDs. Preserve FC and width across profile, inspector, selection and watch paths.
- Reject missing/invalid profile read FC and invalid spans; distinguish missing words from valid zero.
- Breaking prototype project schema v2 and separate config-v2.json; old files unchanged, no automatic migration.
- Monitor uses FC-aware raw watches, including full tag spans; typed trend/log decoding remains PR #14.


### Build / Governance — PR #10 (solo-maintainer release policy)
- Keep protected main and immutable release tags, strict PR-only merges, five CI checks and Windows QA; set required PR approvals to zero, disable last-push and code-owner approval requirements.
- Keep official Release strictly manual, with no independent Environment review or approval sentinel secret; do not publish automatically on tag push.
- Require real repository bench-acceptance Issue for the precise release commit (SHA, PASS, tested hardware, transport and results), in addition to CI and QA, before publishing.
- Provide RU/EN governance instructions and issue template; live GitHub Ruleset must be edited separately by an administrator.

### Build / Security — PR #9 (repository governance and release gates)
- Add importable, active-by-template GitHub Ruleset JSON for protected main (independent PR review, latest-push approval, strict five-job CI) and immutable v* release tags; GitHub Settings activation is required.
- Replace automatic tag-triggered publishing with a manually dispatched, fail-closed release workflow verifying exact main SHA, 5 green CI checks, same-SHA QA artifact, documented bench PASS and active Rulesets.
- Gate final publishing behind a protected `production-release` GitHub Environment with independent reviewers and provisioned sentinel secret; upload EXE/MSI with SHA256SUMS and build metadata.
- Add Governance CI unit tests with negative cases, automatically run Windows QA on main commits, and document activation and acceptance EN/RU.

### Added — PR #7 (OT Safety Policy, stacked on PR #6)
- Fail-closed, explicit IP/CIDR allowlist and exclusion rules shared by GUI, CLI and REST scans; prevent hostname-based policy bypass.
- Quiet Mode global scan TX pacing to 2 requests/sec, including retries, FC43 and register-map traffic; TCP handshake attempts separately paced to 2/sec with effective IP concurrency 1.
- REST/WS API refuses startup without `MODBUS_SCANNER_API_TOKEN` (24+ printable ASCII characters), authenticates every route and rejects cross-origin browser access.
- RU/EN scan UI and CLI flags for OT policy; negative loopback tests for authorization, request pacing and API security.



### Fixed — PR #6 (manual write safety and transport stabilization)
- Manual FC05/FC06/FC10 writes and arbitrary Raw Frames are single-shot regardless of configured retries; only read requests may retry (max 5).
- Empty/oversized raw PDU/ADU are rejected before transmission; receiving an arbitrary raw response is explicitly not treated as proof of a successful write.
- Positive write acknowledgments require exact echo of requested address and value/quantity; mismatched acknowledgments are failures, not false-positive success.
- Timeout/transport failures on writes explicitly warn that the device may have executed the command and that the outcome is unknown.
- RTU-over-TCP accepts delayed first bytes and assembles common/FC43 responses by length instead of prematurely treating a 5ms gap as EOF; bounded TLS handshake.
- RTU/ASCII receive buffers have strict maximum sizes to reject noisy/oversized frames; Windows/Linux/macOS regression tests cover delayed, fragmented and silent responses.


### Fixed — PR #5 (strict Modbus response validation / строгая валидация)
- Rejects truncated/oversized PDU and invalid exception FC/code/length; shared MBAP transaction/protocol/length/unit verification for scanner and manual I/O.
- FC01/02 bit counts must exactly match ceil(requested quantity / 8), without panics; FC03/04 register byte counts and actual quantity must match the request.
- Discovery no longer identifies devices from malformed responses, and invalid manual/monitor/register-map reads no longer produce empty success values.
- Adaptive register scans abort on malformed response rather than recursively splitting potentially thousands of addresses.
- FC43 device identification rejects undeclared trailing bytes; added malformed TCP loopback and RTU/ASCII/UDP regression tests.


### Fixed — PR #4 (scanner lifecycle and cancellation)
- Exclusively guards active scans through Preparing, Running, Paused and Stopping, including concurrent start requests.
- Immediate Stop interrupts IP/serial requests, slot waits and request gaps; aborts outstanding workers and discards planned register reads.
- Stopped status is published only after cleanup, with terminal events consistent in Vue and CLI/API.
- Added no-transmit-after-Stop loopback regression scenarios and cancellation from Pause.

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

## Release status convention / Как читать версии

**Unreleased** entries describe work prepared in development branches or merged toward the next release; they are **not proof of published binaries**. The [latest published release](https://github.com/uzbatsu-beep/Modbus_SCANNER/releases) is the authoritative source of downloadable versions. As observed on 2026-11-21, v1.0.0 is public, `1.0.1` remains in source manifests, and PR #10 is still open. See [current audit](docs/AUDIT_2026-11-21.md).

Раздел **Unreleased** не означает публикацию установщика или подтверждённую приёмку на оборудовании. Для этого проверяются GitHub Releases, конкретный SHA, CI, QA и отдельный протокол физического стенда.

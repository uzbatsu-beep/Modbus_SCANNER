# Development plan

[Русская версия](PLAN.ru.md) · [Research](RESEARCH.md) · [Current status](ROADMAP.md)

**Date:** 2026-11-18 · **Horizon:** four releases (v0.2 → v1.0) ·
**Principle:** every stage delivers a working, safe, verifiable increment.

---

## 1. Goals and constraints

### Product goal
Become a **safe “scanner + interpreter”**: not only find a Modbus device, but
understand what it is, which registers it exposes, how to read them, and save the
result as a ready-to-use tag map.

### Non-negotiable principles
1. **Discovery = read-only.** No write codes in the discovery path. An integration
   test with a mock device asserts zero write requests (Modbus Logic practice).
2. **Explicit user action.** No background activity; scanning, monitoring and
   logging are button-driven and always have Stop/Pause.
3. **Care with OT.** A “quiet” mode with rate limits; authorization and device-load
   warnings; no write tests by default.
4. **No telemetry.** Local data, local files (as in the reference tools).
5. **Custom protocol stack.** Control over timing and framing is our advantage;
   do not replace it with a pymodbus-like library.

---

## 2. Baseline and gaps

Present: RTU/ASCII/TCP/UDP, CRC/LRC/MBAP, unit-ID sweep, IP ranges, Tauri events,
raw register map, manual console, CSV/JSON/XML export, config, four UI tabs, CI,
EN/RU docs, GPL-3.0, Release v0.1.0.

Missing (see [RESEARCH.md](RESEARCH.md)): serial-setting sweep, adaptive register
scan, FC43 identification, typed decoding and byte order, live monitoring, raw
traffic log, quiet mode, IP concurrency, CLI, projects/history/diff, safe auditing.

---

## 3. Phases

| Phase | Theme | Duration* | Key outcome |
|-------|-------|-----------|-------------|
| **v0.2** | Field Discovery | 3–4 weeks | Find a device with unknown settings in one run |
| **v0.3** | Decode & Monitor | 4–6 weeks | Produce a named tag map and verify decoding |
| **v0.4** | Diagnostics & Security (opt-in) | 4–6 weeks | Diagnostics, raw log, PCAP, safety report |
| **v1.0** | Ecosystem & Release | 4 weeks | CLI/headless, profiles, localization, installers |

\* Estimate for one developer at 20–30 h/week.

---

## 4. v0.2 — Field Discovery (P0)

**Goal:** solve the main field-engineer pain — “the device is there, the docs are not”.

| ID | Task | Priority | Rationale (source) | Effort |
|----|------|----------|--------------------|--------|
| P2-01 | Serial-setting sweep: baud 1200…115200 × 8N1/8E1/8O1 (plus 7N1/7E1), early exit on first valid CRC | P0 | Almost every field guide; [ModbusCloud](https://www.modbuscloud.com/knowledge-base/modbus-network-scan) | M |
| P2-02 | Wire `ScanParams` into the engine: timeout/retries/request_gap/ip_concurrency actually take effect | P0 | Our ROADMAP; prerequisite for quiet mode | S |
| P2-03 | Adaptive register scan: Quick/Balanced/Deep, failed block split 100→20→5, exception 01/03 → immediate abort, timeouts not split | P0 | [Modbus Logic](https://www.modbuslogic.com/docs/modbus-scanner/) | L |
| P2-04 | IP-target classification: “valid Modbus” / “port open, no Modbus” / refused / timeout / unreachable | P0 | Same source; open 502 ≠ device | M |
| P2-05 | FC43/MEI14 device identification (basic: VendorName, ProductCode, Revision) | P1 | [FC43](https://www.merobix.com/blog/what-is-modbus-function-code-43-device-identification), [Nmap modbus-discover](https://nmap.org/nsedoc/scripts/modbus-discover.html) | M |
| P2-06 | Bounded-concurrency IP worker pool with a transport per target, `ip_concurrency` cap | P1 | [modbus-probe](https://github.com/isecwire/modbus-probe), [Modbus Connect](https://modbusconnect.com/features) | M |
| P2-07 | Deep scan for TCP/UDP single devices (currently serial only) | P1 | Our ROADMAP | S |
| P2-08 | Per-device export + four address notations (1-based/0-based/reference/hex) in the table | P1 | [Reddit r/PLC](https://www.reddit.com/r/PLC/comments/f2728e/), Modbus Logic | S |

**Exit criteria:** an unknown RTU device is found from any standard baud/parity in
≤ 5 minutes; zero write requests in discovery (test); a /24 IP scan completes in
≤ 30 s at concurrency 32; all UI parameters actually affect the engine.

**Architecture:** new `scanner::strategies::BaudSweepStrategy`, `RegisterScanStrategy`,
`modbus::device_id` (FC43 PDU), `scanner::classifier` (response classification),
extensions to `ScanTarget`/`ScanParams` plus `src/types/ipc.ts` sync.

---

## 5. v0.3 — Decode & Monitor (P1)

**Goal:** turn a “list of raw registers” into a meaningful tag map.

| ID | Task | Priority | Rationale | Effort |
|----|------|----------|-----------|--------|
| P3-01 | Decoder: int16/uint16/int32/uint32/float32/ASCII + 4 byte orders (ABCD, BADC, CDAB, DCBA) | P0 | CAS, Chipkin, Modbus Logic, Proxus decoder | L |
| P3-02 | “Possible float32 (ABCD)” hints always labelled as a guess, never a fact | P1 | Modbus Logic inspector | S |
| P3-03 | Named registers: name, type, byte order, scaling `(raw×mul÷div)+offset`, units, description | P0 | Modbus Logic io-tags | M |
| P3-04 | Projects: devices + tag maps + scan history in one file | P1 | Modbus Logic projects | M |
| P3-05 | Live Monitor: polling 250 ms–10 s, min/max, change highlighting, trend 1 min–1 h | P0 | Modbus Connect, ModbusLens | L |
| P3-06 | Raw TX/RX log with timestamps and FC; CSV/JSON log export | P1 | Modbus Connect | M |
| P3-07 | Logger with rotation (daily/session/size), multiple loggers | P2 | Modbus Connect | M |
| P3-08 | Compare two scans (register map diff) | P2 | Our ROADMAP; Chipkin “register map versioning” | M |
| P3-09 | Full RU/EN localization + light theme | P2 | Our ROADMAP; i18n in modbus-rtu-tool | M |

**Exit criteria:** the pair `0x4367 0x3333` is correctly shown as float32 ≈ 231.2
in every byte order; names and scaling persist in a project; monitoring 10 tags
at 1 s works without leaks or UI blocking.

**Architecture:** `modbus::decode` (pure functions + tests), `state::TagMap`,
a new Pinia `monitor` store, `monitor:update` Tauri events, `TrendChart` component.

---

## 6. v0.4 — Diagnostics & Security (opt-in, P2)

**Goal:** diagnostics for engineers and safe auditing (on request, authorized).

| ID | Task | Effort |
|----|------|--------|
| P4-01 | FC08 Diagnostics (subcodes), FC07/0B/0C — serial only, read-only | M |
| P4-02 | FC support map: probe FC 1–127 in read-only mode; response timings | M |
| P4-03 | PCAP export of Modbus traffic (as in modbus-probe) for Wireshark | M |
| P4-04 | “Quiet” mode: ≤2 requests/s, adaptive backoff, explicit warning | S |
| P4-05 | Passive fingerprinting (traffic listening) as a research prototype | L |
| P4-06 | Optional security report: severity, JSON/CSV, authorization disclaimer | M |
| P4-07 | Write test with rollback — **separate mode only**, double confirmation, off by default | M |

**Important:** implement P4-07 last and only on real demand; research and standards
([IEEE](https://ieeexplore.ieee.org/document/11471618/),
[IEC 62443](https://www.isa.org/standards-and-publications/isa-standards/isa-iec-62443-series-of-standards))
confirm the risk of active impact on OT.

---

## 7. v1.0 — Ecosystem & Release (P2)

- Headless CLI for the same engine (`modbus-scanner scan --rtu COM3 --sweep ... --json`).
- Local REST/WebSocket API for integrations (Modbus Logic ecosystem pattern).
- Device profiles: database of known register maps (CSV/JSON import), auto decoder setup.
- Modbus TLS (port 802) — research task after TCP stabilizes.
- Distributables: Windows MSI, Linux AppImage/deb, macOS dmg; GitHub Release automation.
- Docs: complete EN/RU parity, video tutorial, examples with real devices.

---

## 8. Quality metrics / KPI

| Metric | Target |
|--------|--------|
| Write requests in discovery | **0** (integration test on every PR) |
| Discovery time at unknown baud/parity | ≤ 5 min (247 IDs × 7 baud × 3 parity with early exit) |
| False “devices” | 0 for an open port with no Modbus response |
| Memory for a 65536-register scan | ≤ 200 MB |
| CI | green on Windows + Linux for every commit |
| Protocol-layer test coverage | ≥ 80% lines |
| IPC type sync | 100% (Rust ↔ `src/types/ipc.ts`) |

---

## 9. Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Serial sweep takes hours | Early exit, priority order (9600 8N1 → 2400 → 19200 → 8E1), parallel IDs only after settings are fixed |
| Duplicate addresses cause CRC collisions | Flag suspected collisions, recommend point-to-point; never count a CRC error as a device |
| Device/gateway overload | Quiet mode, request gap, concurrency cap, 300–800 ms default timeouts |
| Two masters on RS-485 | Warning in UI before scanning (per ModbusCloud guide) |
| Rust/TS type drift | Type generator (ts-rs) in v0.3–v0.4 |
| 32-bit float/endianness mistakes | Vector table from real devices; hints labelled as hypotheses |
| Legal risk of security mode | Opt-in, disclaimer, authorization requirement, read-only by default |

---

## 10. Next sprint

1. P2-02 (wire params) → 2. P2-01 (baud sweep) → 3. P2-03 (register scan) →
4. P2-04 (IP classification) → 5. P2-05 (FC43) → 6. P2-08 (per-device export).
Each step is a separate PR with tests and EN/RU doc updates.

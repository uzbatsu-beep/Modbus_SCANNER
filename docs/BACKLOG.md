# Product backlog / Реестр функций

**2026-11-26.** Proposed implementation order under the approved workbench direction. P0 correctness/foundations, P1 core engineering capabilities, P2 differentiated extensions, P3 late-stage platform, R&D research. All items below are planned, except existing baseline mechanisms explicitly noted. IDs are stable; do not reuse. PR numbers in roadmap are indicative.

| ID | Priority | Scope / Результат | Acceptance / Проверка |
|---|---|---|---|
| R02 | P0 | FC-aware register identity and tags | FC03/FC04 at same address remain distinct end-to-end; v2 schema, reject incompatible legacy projects; no implicit FC |
| R01 | P0 | Typed monitor | -5000 INT16 × 0.01 = -50.00; same table/chart/export; invalid data is not zero |
| R04 | P0 | RTT, cycle timing, data quality | Measure monotonic elapsed time; distinguish exception, timeout, invalid response, stale |
| R20 | P0 | Connection manager | Serialize access per physical serial port; independent transports progress separately; stop releases ownership |
| R05 | P1 | Extended types, ENUM, BITFIELD | Golden vectors; exact 64-bit integers through IPC; unknown enum visible |
| R06 | P1 | Profile schema v2 | Versioning, FC, width, provenance, compatibility; lossless round trip |
| R21 | P1 | XLSX/ODS import | Preview column mapping, validation, conflict report; no implicit write |
| R22 | P1 | Visual profile editor | Edit, validate, undo/preview, export/reimport |
| R07 | P1 | Addressing assistant | Explicit decimal/hex, zero/one-based, table conventions; reject overflow |
| R09 | P1 | Single/group/adaptive polling | User-selectable mode; no cross-table or unsupported gap grouping; preserve values |
| R26 | P1 | Bus utilization | Estimated vs measured separated; report bytes, gaps, RTT and cycle statistics |
| R08 | P1 | Multi-device workspace and trends | Concurrent independent sessions, multi-channel views, quality and actual timestamp |
| R19 | P1 | Independent sessions | Independent transport lifecycle and failure isolation; no competing master on serial bus |
| R25 | P1 | Workspace import/export | Versioned project round trip with connections, devices, profiles, views |
| R14 | P1 | Industrial profile library | Licensed/source-attributed maps; verified vs unverified and firmware compatibility |
| R24 | P1 | Before/after comparison | Compare exact identity and typed/raw values; no automatic parameter writes |
| R12 | P1 | Modbus simulator | Profile-driven isolated server; scripted responses; no live-device replay |
| R16 | P1 | Test scenarios and assertions | Deterministic simulator-only tests, expected values/errors, repeatable reports |
| R13 | P2 | Session capture/replay | Offline replay from captured data; timestamps, no transmission by default |
| R11 | P2 | Passive PCAP analysis | Offline authorized capture import, reconstruct TCP transactions; distinguish evidence from inference |
| R10 | P2 | HTML/PDF/XLSX reports | Evidence, source revision, quality, raw/typed values, export traceability |
| R15 | P2 | SunSpec and domain models | Validate against official model definitions; optional profile adapters |
| R23 | P2 | Calculated tags | Deterministic expressions, units, input quality propagation, no writes |
| R27 | P2 | Poll interval assistant | Recommend settings from measured capacity, user confirmation, no hidden changes |
| R28 | P2 | Diagnostic panel builder | Save/import layouts; consistent tags and quality |
| R29 | P2 | IEC 61131-3 export | Generate typed declarations/mapping with explicit addressing, target dialect selectable |
| R03 | P3* | Expanded OT Safety Policy | *Baseline existing safeguards, Stop and resource ownership retained throughout; late corporate policies separate |
| R17 | P3 | OPC UA/MQTT/Sparkplug integration | Opt-in, local first, read-only first, versioned mapping |
| R18 | R&D | Local AI interpretation | Source-linked suggestions, manual confirmation; no automatic device writes |
| R30 | R&D | Documentation-to-profile assistant | Extract candidate map with source/page reference; review required before use |

## Scheduling
R02 → R01 → R04 → R20 → R05/R06 → R21/R22/R07 → R09/R26 → R08/R19/R25 → R14/R24 → R12/R16 → R13/R11/R10/R29. Others enter after core maturity. R03 baseline is cross-cutting, not a separate security overhaul. Re-evaluate dependency graph after each PR; do not treat numbering or effort as a calendar promise. See [roadmap](ROADMAP.md).

# Roadmap / Дорожная карта — Modbus Engineering Workbench

**Revision: 2026-11-26.** Product direction approved; individual implementation contracts and future PR numbering are proposed. [VISION](VISION.md) · [BACKLOG R01–R30](BACKLOG.md) · [Previous roadmap](ROADMAP_LEGACY_2026-11-21.md) · [ADR](adr/README.md).

## Baseline
At inspection on 2026-11-26, main was `7710ef262afb56c1dc18f850a67166ef3cbdfbf1`; PR #10/#11 changes merged, CI and Windows QA successful on that SHA. Published release v1.0.0; manifests v1.0.1; physical bench PASS not established. No release is implied by this strategy PR. See [field validation](FIELD_VALIDATION_v1.0.1.md). Existing discovery, adaptive register scan, FC43, basic decoding, profiles, raw monitor, logs, CLI/API and TLS are foundation, not finished workbench capabilities.

## Priorities
Prototype: correct data, reliable communication, distinctive engineering workflow, useful features, UI and performance. Keep existing baseline controls; advanced security and corporate authorization belong to late stages. CI/bench constraints remain in effect. No need to delay isolated development until a public release; physical acceptance remains necessary before publishing. Do not change versions prematurely.

| Version target | Theme | Scope / Exit gate |
|---|---|---|
| v1.0.1 | Stabilization | Existing candidate, automated and physical validation separate |
| v1.1 | Engineering Core | FC-aware identity, typed monitoring, RTT/quality, connection ownership, extended types; old projects migrate |
| v1.2 | Device Engineering | Profiles v2, import/editor, address assistant, configurable poll planner, measured performance |
| v1.3 | Professional Workbench | Independent sessions, project tree, multi-device trends, profiles and comparison |
| v1.4 | Virtual Engineering Bench | Profile-driven simulator, tests, record/offline replay |
| v1.5 | Advanced Diagnostics | PCAP import, reports, PLC export, advanced diagnostic views |
| v2.0 | Engineering Platform | Optional industry models, integrations, late security work, separately approved R&D |

## Implementation sequence (indicative PR numbering)
| PR | Scope | Backlog | Completion |
|---|---|---|---|
| #12 | Strategy and roadmap | all | design docs and existing CI |
| #13 | Register Identity & Tag Model | R02 | FC/table preserved across storage, import and monitor; migration tests |
| #14 | Typed Monitoring Engine | R01 | signed values, consistent table/trend/log, quality |
| #15 | Communication Metrics | R04 | measured RTT, cycle/period, error categories |
| #16 | Connection & Transport Manager | R20, baseline R03 | serial resource ownership, independent sessions/cancellation |
| #17 | Extended Data Types | R05 | golden vectors, exact 64-bit IPC, ENUM/bitfields |
| #18 | Profile Schema v2 | R06 | versioned schema and round-trip migration |
| #19 | Profile Editor & Spreadsheet Import | R21/R22 | user mapping, preview, validation |
| #20 | Addressing Assistant | R07 | explicit notation and conversion |
| #21 | Polling Engine v2 | R09/R26 | single/group/adaptive selection; benchmark evidence |
| #22 | Multi-device Workspace | R08/R19/R25 | independent sessions, saved workspace, stable tree |
| #23 | Advanced Trends & Oscilloscope | R01/R08 | multiple series, time cursor, quality gaps |
| #24 | Industrial Profiles | R14/R24 | provenance, firmware, comparison |
| #25 | Modbus Simulator | R12 | isolated profile-driven device |
| #26 | Engineering Test Center | R16 | repeatable assertions on virtual bench |
| #27 | Session Recorder & Replay | R13 | capture and offline playback |
| #28 | PCAP Analyzer | R11 | offline authorized import and transaction diagnostics |
| #29 | Engineering Reports | R10/R29 | reproducible exports and typed PLC mapping |

R15, R23, R27, R28 are follow-up P2 features; R03 expanded, R17 and R&D R18/R30 require separate decisions after core maturity. No blind probing of all FC codes; only known read-only functions/subfunctions, with explicit bounds. Do not use line count as refactoring target.

## Design and acceptance references
[DATA_MODEL](DATA_MODEL.md), [DATA_TYPES](DATA_TYPES.md), [PROFILE_SCHEMA](PROFILE_SCHEMA.md), [POLLING_ENGINE](POLLING_ENGINE.md), [UX_DESIGN](UX_DESIGN.md), [TEST_STRATEGY](TEST_STRATEGY.md), [BENCHMARKS](BENCHMARKS.md). Each PR should have explicit dependencies, scope, tests and known limitations, not a promised delivery date.

## Русское резюме
Основной продукт — локальная инженерная среда для АСУ ТП, от обнаружения оборудования до проверенной карты регистров, мониторинга и виртуального стенда. Сначала исправляем потерю Function Code и неверное декодирование, затем измеряем качество связи и строим менеджер соединений. Дальше — типы, профили, импорт, пользовательская настройка опроса, многоприборный интерфейс. Расширенная безопасность, корпоративная авторизация и шлюзы откладываются; базовые ограничения и защита от некорректного обмена сохраняются. Предыдущая редакция сохранена отдельно.

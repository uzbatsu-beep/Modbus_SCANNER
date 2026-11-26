# Roadmap / Дорожная карта — Modbus Engineering Workbench

**Revision: 2026-11-26.** Product direction and UI variant B approved; future implementation contracts remain subject to review. [VISION](VISION.md) · [BACKLOG R01–R30](BACKLOG.md) · [Previous roadmap](ROADMAP_LEGACY_2026-11-21.md) · [ADR](adr/README.md).

## Baseline
Baseline at PR #18 start: main `f466f9c2e881cbcd1f317b9128299ab7e2ff5c93`, PR #12–#17 merged; CI successful; post-merge Windows QA pending at inspection. 17.A is isolated core, not yet used by operational callers. Published v1.0.0 / manifests v1.0.1, physical acceptance not established. FC-aware tags, typed monitor and communication metrics are implemented. See [field validation](FIELD_VALIDATION_v1.0.1.md).

## Priorities
Prototype: correct data, reliable communication, distinctive engineering workflow, useful features, UI and performance. Keep existing baseline controls; advanced security and corporate authorization belong to late stages. CI/bench constraints remain in effect. No need to delay isolated development until a public release; physical acceptance remains necessary before publishing. Do not change versions prematurely.

| Version target | Theme | Scope / Exit gate |
|---|---|---|
| v1.0.1 | Stabilization | Existing candidate, automated and physical validation separate |
| v1.1 | Engineering Core | FC-aware identity, typed monitoring, RTT/quality, connection ownership, extended types; project schema v2 (clean prototype break) |
| v1.2 | Device Engineering | Profiles v2, import/editor, address assistant, configurable poll planner, measured performance |
| v1.3 | Professional Workbench | Independent sessions, project tree, multi-device trends, profiles and comparison |
| v1.4 | Virtual Engineering Bench | Profile-driven simulator, tests, record/offline replay |
| v1.5 | Advanced Diagnostics | PCAP import, reports, PLC export, advanced diagnostic views |
| v2.0 | Engineering Platform | Optional industry models, integrations, late security work, separately approved R&D |

## Implementation sequence (indicative PR numbering)
| PR | Scope | Backlog | Completion |
|---|---|---|---|
| #12 | Strategy and roadmap | all | design docs and existing CI |
| #13 | Register Identity & Tag Model | R02 | FC/table preserved across storage, import and monitor; v2 round-trip and rejection tests |
| #14 | Typed Monitoring Engine | R01 | signed values, consistent table/trend/log, quality |
| #15 | Communication Metrics | R04 | measured RTT, cycle/period, error categories |
| #16 | Workbench Shell & Design System | UI-01/UI-02 | approved variant B; resizable panels, compact RU/EN, themes, persistence, existing workflows |
| #17 | 17.A Connection Core | R20 | merged, isolated worker core |
| #18 | 17.B Functional Integration | R20/R04/R03 | approved B.1–B.4; [contract](FUNCTIONAL_INTEGRATION.md) |
| #19 | 17.C Connection Manager UI | UI-03/R20 | editor, persistence, selectors and lifecycle controls |
| #20 | Project Explorer & Workspace Manager | UI-03/UI-04, R08/R19/R25 | stable objects and independent session tabs |
| #21 | Extended Data Types | R05 | exact 64-bit IPC, ENUM/bitfields |
| #22 | Profile Schema v2 | R06 | validated versioned schema |
| #23 | Profile Studio & Spreadsheet Import | UI-10/R21/R22 | preview and validation |
| #24 | Addressing Assistant & Register Inspector | UI-05/R07 | explicit notation and conversion |
| #25 | Polling Engine v2 | R09/R26 | single/group/adaptive and benchmarks |
| #26 | Monitoring & Diagnostics Workspaces | UI-06/UI-07/UI-08, R08/R19/R25 | saved workspace and real independent sessions |
| #27 | Advanced Trends & Oscilloscope | UI-11/R01/R08 | multiple series, cursor, quality gaps |
| #28 | Industrial Profiles | R14/R24 | provenance, firmware, comparison |
| #29 | Modbus Simulator | UI-12/R12 | isolated profile-driven device |
| #30 | Engineering Test Center | UI-12/R16 | repeatable assertions |
| #31 | Session Recorder & Replay | R13 | capture and offline playback |
| #32 | PCAP & Traffic Analyzer | UI-09/R11 | offline authorized import and diagnostics |
| #33 | Engineering Reports & PLC Export | R10/R29 | reproducible exports and PLC mapping |

UI-13 advanced docking/layout manager remains a later optional stage. PR numbering after #19 is indicative; backlog R identifiers remain stable. See [WORKBENCH_SHELL](WORKBENCH_SHELL.md).

R15, R23, R27, R28 are follow-up P2 features; R03 expanded, R17 and R&D R18/R30 require separate decisions after core maturity. No blind probing of all FC codes; only known read-only functions/subfunctions, with explicit bounds. Do not use line count as refactoring target.

## Design and acceptance references
[DATA_MODEL](DATA_MODEL.md), [DATA_TYPES](DATA_TYPES.md), [PROFILE_SCHEMA](PROFILE_SCHEMA.md), [POLLING_ENGINE](POLLING_ENGINE.md), [UX_DESIGN](UX_DESIGN.md), [TEST_STRATEGY](TEST_STRATEGY.md), [BENCHMARKS](BENCHMARKS.md). Each PR should have explicit dependencies, scope, tests and known limitations, not a promised delivery date.

## Русское резюме
Основной продукт — локальная инженерная среда для АСУ ТП, от обнаружения оборудования до проверенной карты регистров, мониторинга и виртуального стенда. Сначала исправляем потерю Function Code и неверное декодирование, затем измеряем качество связи и строим менеджер соединений. Дальше — типы, профили, импорт, пользовательская настройка опроса, многоприборный интерфейс. Расширенная безопасность, корпоративная авторизация и шлюзы откладываются; базовые ограничения и защита от некорректного обмена сохраняются. Предыдущая редакция сохранена отдельно.

# Roadmap / Дорожная карта — Modbus Engineering Workbench

**Revision: 2026-11-26.** Product direction and UI variant B approved; future implementation contracts remain subject to review. [VISION](VISION.md) · [BACKLOG R01–R30](BACKLOG.md) · [Previous roadmap](ROADMAP_LEGACY_2026-11-21.md) · [ADR](adr/README.md).

## Baseline
Baseline at PR #25 start: main `47016ed905149b6e98336a016707cd72a33c31fd`, PR #12–#24 merged; Windows-first main CI and Windows QA passed. C.2.2–C.2.4 merged, C.2.5 automated checkpoint passed; remaining SR acceptance carried into C.3/C.4. Published v1.0.0 / manifests v1.0.1, physical acceptance not established. FC-aware tags, typed monitor and communication metrics are implemented. See [field validation](FIELD_VALIDATION_v1.0.1.md).

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
| #18 | 17.B.1 Communication Service | R20/R03 | merged, validated service foundation |
| #19 | 17.B.2.1 Shared Runtime Foundation | R20/R03 | merged, intermediate serial exclusion and Manual worker |
| #20 | 17.B.2.2 Scanner Network & FC43 | R20/R03 | merged, managed network discovery and FC43 |
| #21 | 17.B.2.3 C.1 Shared Connection Lifecycle | R20/R03 | merged, shared lease and recovery foundation |
| #22 | C.2.1 Exclusive Serial Session | R20/R03 | merged, foundation and seven regressions |
| #23 | Windows-first CI policy | R20 | merged; routine Windows validation, final full-platform gate |
| #24 | C.2.2–C.2.5 Serial Migration | R20/R03 | merged; automated checkpoint only, remaining SR acceptance carried forward |
| #25 | C.3 Protocol & Cross-Tool Integration | R20/R03 | approved C.3.1–C.3.7, [contract](CROSS_TOOL_INTEGRATION.md); foundation checkpoint in progress |
| #26 | C.4 B.2 Integration Acceptance | R20/R03 | full B.2 regression and acceptance before B.3 |
| #27 | 17.B.3 Independent Monitoring | R20/R01 | independent per-connection monitor loops and scoped stop |
| #28 | 17.B.4 Diagnostics & Regression | R04/R20 | queue metrics, Logger and integration acceptance |
| #29 | 17.C Connection Manager UI | UI-03/R20 | editor, persistence, selectors and lifecycle controls |
| #30 | Project Explorer & Workspace Manager | UI-03/UI-04, R08/R19/R25 | stable objects and independent session tabs |
| #31 | Extended Data Types | R05 | exact 64-bit IPC, ENUM/bitfields |
| #32 | Profile Schema v2 | R06 | validated versioned schema |
| #33 | Profile Studio & Spreadsheet Import | UI-10/R21/R22 | preview and validation |
| #34 | Addressing Assistant & Register Inspector | UI-05/R07 | explicit notation and conversion |
| #35 | Polling Engine v2 | R09/R26 | single/group/adaptive and benchmarks |
| #36 | Monitoring & Diagnostics Workspaces | UI-06/UI-07/UI-08, R08/R19/R25 | saved workspace and real independent sessions |
| #37 | Advanced Trends & Oscilloscope | UI-11/R01/R08 | multiple series, cursor, quality gaps |
| #38 | Industrial Profiles | R14/R24 | provenance, firmware, comparison |
| #39 | Modbus Simulator | UI-12/R12 | isolated profile-driven device |
| #40 | Engineering Test Center | UI-12/R16 | repeatable assertions |
| #41 | Session Recorder & Replay | R13 | capture and offline playback |
| #42 | PCAP & Traffic Analyzer | UI-09/R11 | offline authorized import and diagnostics |
| #43 | Engineering Reports & PLC Export | R10/R29 | reproducible exports and PLC mapping |

UI-13 advanced docking/layout manager remains a later optional stage. PR numbering after #23 is indicative; backlog R identifiers remain stable. See [WORKBENCH_SHELL](WORKBENCH_SHELL.md).

R15, R23, R27, R28 are follow-up P2 features; R03 expanded, R17 and R&D R18/R30 require separate decisions after core maturity. No blind probing of all FC codes; only known read-only functions/subfunctions, with explicit bounds. Do not use line count as refactoring target.

## Design and acceptance references
[DATA_MODEL](DATA_MODEL.md), [DATA_TYPES](DATA_TYPES.md), [PROFILE_SCHEMA](PROFILE_SCHEMA.md), [POLLING_ENGINE](POLLING_ENGINE.md), [UX_DESIGN](UX_DESIGN.md), [TEST_STRATEGY](TEST_STRATEGY.md), [BENCHMARKS](BENCHMARKS.md). Each PR should have explicit dependencies, scope, tests and known limitations, not a promised delivery date.

## Русское резюме
Основной продукт — локальная инженерная среда для АСУ ТП, от обнаружения оборудования до проверенной карты регистров, мониторинга и виртуального стенда. Сначала исправляем потерю Function Code и неверное декодирование, затем измеряем качество связи и строим менеджер соединений. Дальше — типы, профили, импорт, пользовательская настройка опроса, многоприборный интерфейс. Расширенная безопасность, корпоративная авторизация и шлюзы откладываются; базовые ограничения и защита от некорректного обмена сохраняются. Предыдущая редакция сохранена отдельно.

Windows-first policy approved after PR #22: Windows native CI/QA during iterative development; Linux/macOS native checks deferred to explicit final CI, followed by final platform packaging and verification. See [release gates](RELEASE_GATES.md).

Development Ruleset: require Frontend, Governance and Windows backend only; remove Linux/macOS required contexts in live settings. Final acceptance restores both via main-final.json and requires actual successful manual full CI on exact main SHA.

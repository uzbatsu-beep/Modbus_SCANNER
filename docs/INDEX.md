# Modbus Scanner — documentation / Навигация по документации

> **Documentation baseline / Срез документации: 2026-11-21.**
> Released version / Опубликованная версия: **v1.0.0**.
> Repository manifests target / Версия манифестов: **v1.0.1 (unreleased)**.
> CI, Windows QA and protocol simulator do **not** establish physical RS-485 acceptance.
> CI, Windows QA и симулятор **не заменяют** испытания на реальном оборудовании.
>
> [Live repository](https://github.com/uzbatsu-beep/Modbus_SCANNER) ·
> [Current audit](AUDIT_2026-11-21.md) ·
> [GitHub releases](https://github.com/uzbatsu-beep/Modbus_SCANNER/releases) ·
> [Open PRs](https://github.com/uzbatsu-beep/Modbus_SCANNER/pulls)

## Start here / С чего начать

| Topic / Тема | English | Русский |
| --- | --- | --- |
| Product / Обзор | [README](../README.md) | [README](../README.ru.md) |
| Development / Разработка | [DEVELOPMENT](DEVELOPMENT.md) | [DEVELOPMENT.ru](DEVELOPMENT.ru.md) |
| Architecture / Архитектура | [ARCHITECTURE](ARCHITECTURE.md) | [ARCHITECTURE.ru](ARCHITECTURE.ru.md) |
| API & IPC | [API](API.md) | [API.ru](API.ru.md) |
| Modbus protocol / Протокол | [MODBUS](MODBUS.md) | [MODBUS.ru](MODBUS.ru.md) |
| Scanning / Сканирование | [SCANNING](SCANNING.md) | [SCANNING.ru](SCANNING.ru.md) |
| Windows QA | [WINDOWS_QA](WINDOWS_QA.md) | [WINDOWS_QA.ru](WINDOWS_QA.ru.md) |
| Release gates / Допуск к релизу | [RELEASE_GATES](RELEASE_GATES.md) | [RELEASE_GATES.ru](RELEASE_GATES.ru.md) |
| Roadmap / Дорожная карта | [ROADMAP](ROADMAP.md) | [ROADMAP](ROADMAP.md) |
| Old milestone plan / Исторический план v0.2–v1.0 | [PLAN](PLAN.md) | [PLAN.ru](PLAN.ru.md) |
| v0/v1 research / Исследование | [RESEARCH](RESEARCH.md) | [RESEARCH.ru](RESEARCH.ru.md) |
| v2 research / Исследование v2 | [RESEARCH_V2](RESEARCH_V2.md) | [RESEARCH_V2.ru](RESEARCH_V2.ru.md) |

## Verification and governance / Проверки и управление

- [Current Git audit / Текущий аудит](AUDIT_2026-11-21.md) — dated evidence and unresolved risks.
- [Historical project audit / История аудита](AUDIT.md) — earlier snapshot, not current live GitHub status.
- [Physical bench matrix / Стендовые испытания v1.0.1](FIELD_VALIDATION_v1.0.1.md) — acceptance to be completed by operator.
- [Offline simulator / Симулятор](TEST_SIMULATOR.md) — deterministic TCP loopback; no physical bus assertions.
- [Security and OT safeguards / Безопасность](SECURITY.md).
- [Changelog / История изменений](../CHANGELOG.md).
- [Contribution guide / Руководство по участию](../CONTRIBUTING.md).
- [Pull-request checklist](../.github/PULL_REQUEST_TEMPLATE.md).
- [CI workflow](../.github/workflows/ci.yml),
  [Windows QA workflow](../.github/workflows/windows-qa.yml),
  [manual release workflow](../.github/workflows/release.yml).

## Source-of-truth order / Приоритет источников

1. **Live GitHub**: actual branch protections, PR status, workflow runs and published assets.
2. **Current code** at the exact SHA: behavioral guarantees and test coverage.
3. **Current documentation**: user guidance tied to that SHA.
4. **Proposals and historical documents**: ROADMAP, PLAN, market research; not proof of implementation.

1. **Настройки GitHub** важнее JSON-шаблона Ruleset в Git.
2. **Исполняемый код и тесты** важнее описания функции в плане.
3. **Протокол реального стенда** обязателен отдельно от автоматических тестов.
4. **Предложения и старые планы** не означают, что функция реализована.

## Maintenance discipline / Правила сопровождения

- Change behavior and its matching RU/EN docs in **one PR**, keeping version, examples and caveats aligned.
- Use [ROADMAP](ROADMAP.md) for future work, [CHANGELOG](../CHANGELOG.md) for changes, [AUDIT_2026-11-21](AUDIT_2026-11-21.md) for a dated snapshot.
- Update dated audit snapshots only by publishing a **new** snapshot. Never silently replace historic observations.
- Validate relative Markdown links with `npm run docs:check`; it performs no network calls.
- A passing Markdown check only verifies paths, not technical truth or external URLs.

**EN:** All scanning requires the asset owner's authorization; use an isolated, non-production bench for release testing.

**RU:** Сканирование допускается только с разрешения владельца оборудования; испытания релиза проводятся на изолированном стенде.

## Engineering Workbench strategy (2026-11-26) / Новая концепция

- [VISION](VISION.md) — product direction and prototype priorities.
- [BACKLOG](BACKLOG.md) — R01–R30 and acceptance criteria.
- [ROADMAP](ROADMAP.md) — current implementation sequence; [previous roadmap](ROADMAP_LEGACY_2026-11-21.md) is historical.
- [DATA_MODEL](DATA_MODEL.md), [DATA_TYPES](DATA_TYPES.md), [PROFILE_SCHEMA](PROFILE_SCHEMA.md) — target data contracts.
- [POLLING_ENGINE](POLLING_ENGINE.md), [UX_DESIGN](UX_DESIGN.md) — target multi-device engineering workflow.
- [TEST_STRATEGY](TEST_STRATEGY.md), [BENCHMARKS](BENCHMARKS.md), [ADR](adr/README.md), [AGENTS](../AGENTS.md) — implementation discipline.

These are target specifications, not claims of features already implemented. Existing protocol/API references remain authoritative for the current runtime. The live governance and release state must be checked independently of historical snapshots.

- [MEASUREMENT_ENGINE](MEASUREMENT_ENGINE.md) — PR #14 typed observations, ring buffers, IPC and Logger v2.

- [COMMUNICATION_METRICS](COMMUNICATION_METRICS.md) — PR #15 measured stages, RTT distributions, cycle and stale diagnostics.

- [WORKBENCH_SHELL](WORKBENCH_SHELL.md) — approved UI shell, layout and acceptance.

- [CONNECTION_MANAGER](CONNECTION_MANAGER.md) — PR #17 phases, resource ownership and acceptance.

- [FUNCTIONAL_INTEGRATION](FUNCTIONAL_INTEGRATION.md) — PR #18 approved B.1–B.4 contract.

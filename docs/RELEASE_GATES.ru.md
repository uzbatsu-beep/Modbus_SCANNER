# Защита GitHub и ручной релиз — индивидуальная разработка

[English version](RELEASE_GATES.md) · [Windows QA](WINDOWS_QA.ru.md) · [Протокол стенда](FIELD_VALIDATION_v1.0.1.md)

## Принятая схема

Разработчик самостоятельно согласовывает PR и официальный релиз **без второго
обязательного участника**. При этом сохраняются: обязательный PR, пять CI
проверок, актуальная ветка `main`, защита тегов `v*`, успешный Windows QA
**на том же SHA**, документированный PASS на физическом стенде и **ручной**
запуск Release. Автопубликации по push тега нет.

`.github/rulesets/*.json` — шаблоны. Фактические правила GitHub нужно
редактировать в Settings. Сейчас главное правило было включено в PR #9
с обязательным одобрением; для индивидуальной разработки его требуется
**изменить непосредственно в настройках**. Подключённый GitHub не позволяет
мне обновить административные настройки Rulesets.

## 1. Изменить действующий Ruleset для main

Откройте [Ruleset main](https://github.com/uzbatsu-beep/Modbus_SCANNER/rules/27728708)
(Settings → Rules → Rulesets). Изменяйте **существующее** правило, не создавая
второе.

В разделе **Require a pull request before merging**:

- Оставить обязательность **Pull request** включённой.
- **Required approvals = 0**.
- Выключить **Require approval of the most recent reviewable push**.
- Выключить **Require review from Code Owners**.
- Выключить **Require extra approval for unattributed changes**, если есть.
- Можно отключить сброс устаревших одобрений: они больше не обязательны.
- Оставить **Require conversation resolution before merging**.

Ни в коем случае не отключать: **Active**, охват `main`, запрет удаления и
force-push, пустой bypass, актуальность ветки перед слиянием и пять
обязательных проверок:

```text
Governance (release gate tests)
Frontend (typecheck + build)
Backend (cargo check + tests) (windows-latest)
Backend (cargo check + tests) (ubuntu-latest)
Backend (cargo check + tests) (macos-latest)
```

Защиту тегов [Ruleset v*](https://github.com/uzbatsu-beep/Modbus_SCANNER/rules/27728718)
оставить **без изменений**: Active; запрещены удаление, любое обновление и
force-push. Новый тег допускается только после ручного release-gate.

Проверьте: `main` по-прежнему protected, PR с незавершённым CI нельзя слить,
но подтверждение второго разработчика не требуется. Не отключайте Ruleset
даже временно и не применяйте bypass.

## 2. Environment с независимым reviewer не нужен

Новый workflow Release **не использует** `production-release` и секрет
`OT_RELEASE_APPROVAL_GATE`. Создавать Required reviewers / Prevent self-review
для выпуска не требуется. Если окружение уже было создано, оно больше
не участвует в процедуре.

Выпуск запускается вами вручную в
**Actions → Release (manual, verified) → Run workflow**. Далее три этапа:
preflight → сборка → публикация. Любая ошибка блокирует создание релиза.

## 3. Стендовая приёмка: GitHub Issue

Перед выпуском:

1. Дождаться CI (5/5) и Windows QA на **точном финальном SHA main**.
   Артефакт QA ветки PR не подходит.
2. Открыть **Issues → New issue → Release bench acceptance / Стендовая приёмка**
   и зафиксировать реальные испытания на изолированном оборудовании.
3. Заполнить в тексте Issue отдельными строками, **без Markdown-префикса**:

```text
Source SHA: <40-символьный SHA текущего main>
Bench result: PASS
Hardware: <модель и прошивка>
Transport: <параметры Modbus RTU/ASCII/TCP>
Test summary: <фактические результаты>
```

4. Приложить дату, исполнителя, протокол, логи и результаты: обычные чтения,
   исключения Modbus, таймауты, CRC/MBAP, Stop, Quiet Mode и операции записи
   только на изолированном стенде. При неудаче указать `FAIL` или `PENDING`,
   релиз будет заблокирован.
5. Preflight запрашивает **реальное Issue** через GitHub API, проверяет URL,
   точный SHA, `PASS`, заполненные поля оборудования и результата. Это
   подтверждение инженера, а **не автоматическое доказательство испытаний**.

## 4. Официальный релиз

На ветке `main` выбрать *Release (manual, verified)* и заполнить:

- `tag`: `vX.Y.Z`, совпадает с npm/Cargo/Tauri и ещё не существует.
- `qa_run_id`: успешный Windows QA на финальном SHA `main`.
- `bench_passed`: `true` **только** после физической приёмки.
- `bench_evidence_url`: ссылка на GitHub Issue данного репозитория.

Проверяются: текущий SHA, обязательный PR и пять CI-checks, QA-артефакт,
активные Rulesets, Issue со стендовой приёмкой и отсутствие тега/релиза.
Windows build повторно выполняет тесты, создаёт EXE/MSI,
`SHA256SUMS.txt` и `BUILD-INFO.txt`. Перед публикацией все требования
перепроверяются, а SHA256 файлов сверяются. Релиз остаётся ручным.

## 5. Ограничения

Шаблон JSON не изменяет действующий GitHub Ruleset. Пока фактическая
настройка продолжает требовать 1 одобрение, ваши PR могут быть заблокированы,
а после слияния нового кода Release preflight также отклонит эту конфигурацию.
Изменить живой Ruleset может администратор репозитория.

Подтверждение PASS не заменяет проверку на конкретном оборудовании,
не является сертификацией IEC 62443 и не гарантирует одинаковую работу
разных преобразователей, ПЛК или Modbus-шлюзов.

## Контроль реальных настроек — 21.11.2026

**Важно:** эта страница описывает **предлагаемую в PR #10 политику**. На момент аудита действующий Ruleset `main` ID `27728708` по-прежнему содержит `required_approving_review_count = 1` и `require_last_push_approval = true`; защита тегов `v*` ID `27728718` активна. Самостоятельное слияние без reviewer пока **не подтверждено** и требует изменения действующих настроек. Стендовый PASS в рамках этого аудита не получен. См. [аудит](AUDIT_2026-11-21.md) и [карту документации](INDEX.md).

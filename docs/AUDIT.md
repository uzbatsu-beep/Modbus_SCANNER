# Аудит проекта / Project audit

**Дата:** 2026-11-19 · **Область:** git, версии, CI/CD, надёжность, оптимизация,
документация и прозрачность · **Статус:** auditors' findings + применённые исправления.

---

## 1. Git и выпуски / Git and releases

| Наблюдение | Severity | Status |
|------------|----------|--------|
| `main` чист, `origin/main` синхронизирован (`6f60df4`) | — | ✅ |
| Теги `v0.1.0`, `v0.1.1` указывают на старые коммиты; весь код v1.0 живёт в `main` без тега | **Medium** | ⚠ recommending `v1.0.0` |
| Версии в `package.json`, `Cargo.toml`, `tauri.conf.json` — `0.1.1`, хотя PLAN/ROADMAP объявляют v1.0 завершённым | **Medium** | ✅ README уточнён; релиз — следующий шаг |
| `.gitignore` корректно исключает `node_modules/`, `dist/`, `target/`, `gen/`; `Cargo.lock` закоммичен | — | ✅ |
| Release workflow срабатывает по тегу `v*` и загружает MSI/NSIS | — | ✅ |

## 2. Надёжность и оптимизация / Reliability & performance

| Наблюдение | Severity | Status |
|------------|----------|--------|
| `ScannerHandle::progress()` двойного учитывал таймауты (`requests_sent + timeouts`) | Medium | ✅ исправлено: `done` = сумма завершённых ответов/ошибок, `total` = `requests_sent` |
| `send_raw_frame` доступен без политики авторизации (риск pass-through по IEC 62443) | **High** | ⚠ запланировано G-01; сейчас только документальное предупреждение |
| Локальный REST/WS API слушает `127.0.0.1` без авторизации — это осознанное решение для desktop | Low | ✅ задокументировано в API.md |
| Классификация `connection_refused` на smoke-тесте `127.0.0.1:1` вернула `timeout` | Low | ⚠ требует проверки маппинга `io::ErrorKind` на Windows |
| Тайминги (`timeout_ms`, `gap`, `retries`) уже проброшены в движок; IP-пул ограничен `ip_concurrency` | — | ✅ |
| Единый `EventSink` отделяет ядро от Tauri; mock-транспорт и CI на 3 ОС | — | ✅ |
| Генерация TS-типов проверяется в CI (`git diff --exit-code`) | — | ✅ |

## 3. Документация и прозрачность / Documentation

| Наблюдение | Severity | Status |
|------------|----------|--------|
| ARCHITECTURE описывала «четыре вкладки» и упрощённый `AppState` | Low | ✅ обновлено: 7 view + все handle'ы |
| MODBUS таблица заявляла «поддержка в кодеке» для FC0x16/0x17, хотя есть только перечисление в `FunctionCode` | Low | ✅ исправлено на «reserved only» |
| `package.json` содержал сломанный скрипт `lint` (eslint не установлен) | Low | ✅ удалён; `npm run typecheck` — рабочая проверка |
| Комментарий в `Cargo.toml` ссылался на несуществующий feature `mobile-libs` | Low | ✅ исправлено: ручная замена `crate-type` для мобильных таргетов |
| Пары EN/RU есть для README, ARCHITECTURE, API, MODBUS, SCANNING, DEVELOPMENT, PLAN, RESEARCH, RESEARCH_V2 | — | ✅ |
| SECURITY/ROADMAP/CHANGELOG — двуязычные в одном файле | Low | ⚠ допустимо, но асимметрично с парами |

## 4. Рекомендации / Next actions

1. **Создать тег `v1.0.0` и run release workflow** — закрепить код v1.0 в релизе с инсталляторами; после этого обновить README/CHANGELOG.
2. **G-01 policy engine** — превратить «read-only по умолчанию», quiet mode и подтверждение авторизации в исполняемые политики, ограничить `send_raw_frame`.
3. **Проверить маппинг `connection_refused`** на Windows в `classify_ip_error` и добавить юнит-тест.
4. **По желанию** — добавить ESLint + конфиг или явно зафиксировать отказ от линтера в DEVELOPMENT (сейчас удалён сломанный скрипт).

## 5. English summary

Git is clean and synced; CI covers Windows/Linux/macOS plus generated-binding
freshness and release automation. The main transparency gap is that v1.0 code is
already in `main` while package version and published release are still `0.1.1`
— recommend a `v1.0.0` tag. Reliability fixes applied in this audit: progress
double-counting, stale architecture/protocol docs, a broken `lint` script and a
misleading Cargo comment. Open items: policy-gated raw writes (G-01) and
verification of `connection_refused` classification on Windows.

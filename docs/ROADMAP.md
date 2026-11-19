# Дорожная карта / Roadmap

> Обоснование приоритетов и рыночный контекст: [RESEARCH.md](RESEARCH.md) /
> [RESEARCH.ru.md](RESEARCH.ru.md); исследование платформы v2.0 и выбор
> библиотек (§9):
> [RESEARCH_V2.md](RESEARCH_V2.md) / [RESEARCH_V2.ru.md](RESEARCH_V2.ru.md).
> Детальные задачи и критерии готовности выполненных этапов: [PLAN.md](PLAN.md) /
> [PLAN.ru.md](PLAN.ru.md).
> История изменений: [CHANGELOG.md](../CHANGELOG.md).

**Дата обновления:** 2026-11-19 · **Текущий релиз:** v1.0.0 · **Ветка:** `main`

---

## 1. Где мы сейчас / Current status

| Этап | Статус | Что дал |
|------|--------|---------|
| **v0.1.0** — MVP | ✅ релиз | Протокол RTU/ASCII/TCP/UDP, транспорты, движок сканера, UI, экспорт, документация |
| **v0.2** — Field Discovery | ✅ в v0.1.1 | Автосвип baud/чётности, адаптивный register scan, классификация IP, FC43, экспорт по устройству, валидация MBAP tx id, выбор ADU, журнал зондов |
| **v0.3** — Decode & Monitor | ✅ в v0.1.1 | Декодер типов и byte order, именованные теги, проекты с историей, live-мониторинг и тренды, raw TX/RX-журнал, файловые логгеры, diff сканов, RU/EN UI и светлая тема |
| **v1.0.0** — Ecosystem & Automation | ✅ релиз | CLI, локальный API, профили устройств, Modbus TLS, автоматизация релизов, генерация типов, тесты и CI на 3 ОС; MSI + NSIS в GitHub Release |
| **v1.1** — Diagnostics | 📋 следующий | FC08/07/0B/0C, FC support map, автоопределение диапазонов, bitfields, XLSX, пассивный fingerprinting, quiet mode |
| **v1.2** — Workflow | 📋 запланирован | Шаблоны и расписание сканов, отчёты, автосохранение, hotkeys, доступность |
| **v2.0** — Platform | 💡 исследование + выбор стека | Шлюзы MQTT/OPC UA, web-компаньон, политики IEC 62443, плагины; BACnet отложен (см. §9 RESEARCH_V2) |

**Качество на текущий момент:** `cargo check` и `npm run tauri build` — без предупреждений; CI зелёный на Windows / Linux / macOS; Windows-инсталляторы (NSIS + MSI) загружены в Release v1.0.0; все 6 проверок релизного workflow зелёные.

---

## 2. Принципы (не меняются от релиза к релизу)

1. **Обнаружение = только чтение** — 0 write-запросов в контуре discovery, проверяется тестом.
2. **Никакой фоновой активности и телеметрии** — всё запускается пользователем, данные остаются локальными.
3. **Явная авторизация для OT** — «тихий» режим и предупреждения при сканированииindustrial-сетей.
4. **Собственный протокольный стек** — контроль таймингов и фрейминга, не заменять на pymodbus-подобную библиотеку.
5. **Синхронизация контрактов** — Rust IPC-типы и `src/types/ipc.ts` меняются в одном PR.
6. **Документация EN/RU в том же PR** — иначе фича не считается готовой.

---

## 3. v1.0 — Ecosystem & Automation (4–6 недель, приоритет P0)

**Цель:** сделать движок доступным не только из GUI — для автоматизации, интеграций и безопасного подключения к совремённым сетям.

| ID | Задача | Почему (источник) | Оценка | Зависимости |
|----|--------|-------------------|--------|-------------|
| E-01 ✅ | Выделить ядро движка от `tauri::AppHandle`: событийный адаптер (`EventSink`) вместо прямого `app.emit` | Готово: `EventSink`, `TrafficSink`, `SampleSink` и `bridge.rs`; ядро не содержит `tauri::` | L | — |
| E-02 ✅ | Headless CLI: `modbus-scanner scan --port COM3 --auto --output report.json` | Готово: JSON в stdout/файл, serial-auto и TCP/UDP-цели, общий движок через `EventSink` | M | E-01 |
| E-03 ✅ | Локальный REST + WebSocket API (`127.0.0.1`, без авторизации по умолчанию, только read) | Готово: axum, `/health`, `/api/v1/*`, WS-поток событий, CLI `serve` и Tauri-команды | L | E-01 |
| E-04 ✅ | Профили устройств: библиотека известных register map, импорт CSV/JSON, автоподстановка декодера по FC43 | Готово: `DeviceProfile`, импорт CSV/JSON, match по identity, вкладка Profiles и подсказка в карточке | M | P3-01, P3-04 |
| E-05 ✅ | Modbus Security (TLS, порт 802, X.509) — исследование и MVP-поддержка | Готово: `tokio-rustls`, MBAP в TLS, webpki CA, mTLS, CLI `--tls`, UI-чекбокс | L | — |
| E-06 ✅ | Автоматизация Release: пересборка инсталляторов и загрузка ассетов в GitHub Actions | Готово: `release.yml` по тегу `v*`, Windows build → artifact → GH Release | S | — |
| E-07 ✅ | Генерация `src/types/ipc.ts` из Rust (`ts-rs` или свой генератор схем) | Готово: 67 файлов в `src/types/generated`, фасад, `npm run gen:types`, CI-проверка | M | — |
| E-08 ✅ | Юнит-тесты стратегий/движка с mock-транспортом; CI-матрица Linux/macOS | Готово: `MockTransport`, тесты конвейера build→transact→verify, CI на Windows/Linux/macOS | M | E-01 |

**Критерии выхода v1.0:** CLI собирает отчёт без GUI; API отвечает на `/health` и `/scan`; профиль применяется к найденному устройству по Vendor/Product; релиз публикуется кнопкой в CI; все IPC-типы генерируются; тесты проходят на 3 ОС.

---

## 4. v1.1 — Diagnostics & Engineering (4 недели, приоритет P1)

| ID | Задача | Обоснование |
|----|--------|-------------|
| D-01 | FC08 Diagnostics + FC07/0B/0C (только serial, read-only), журнал событий и счётчиков | Производственная диагностика шины |
| D-02 | FC support map: опрос FC 1–127 в режиме «только чтение», тайминги | Аналог `modbus-probe` без write-тестов |
| D-03 | Автоопределение непрерывных диапазонов регистров с настраиваемым шагом | Ускорение разбора незнакомой карты |
| D-04 | Пресеты декодирования: float32/uint32/int32/bitfields, scaling-шаблоны | [RESEARCH](RESEARCH.md): самый частый запрос после обнаружения |
| D-05 | CSV/XLSX-экспорт с декодированными колонками и единицами | Готовность отчёта для инженера |
| D-06 | Пассивный fingerprinting (прослушивание трафика) как исследовательский прототип | IEEE 11471618, MOSTO: активное сканирование OT опасно |
| D-07 | «Тихий» режим с жёстким rate-limit и проверкой авторизации | IEC 62443, NIS2 |

**Критерии выхода v1.1:** 0 write-запросов в контуре discovery (интеграционный
тест в CI); FC support map для одного устройства строится ≤ 60 с; XLSX открывается
в Excel/LibreOffice; quiet mode детерминированно ограничивает частоту опроса и
требует явного подтверждения пользователя.

---

## 5. v1.2 — Workflow & Reliability (3–4 недели, приоритет P1)

- Шаблоны сканов и расписание (cron-like) с запуском через CLI/API.
- Рабочее пространство из нескольких проектов, сравнение проектов между файлами.
- Генератор отчётов HTML/PDF с графиками трендов и сравнением сканов (diff).
- Автосохранение результатов после каждого цикла, восстановление после сбоя.
- Горячие клавиши, доступность (ARIA, контраст светлой темы).
- Ротация диагностических логов самого приложения.

---

## 6. v2.0 — Platform (8–12 недель, приоритет P2)

Исследование завершено, включая выбор библиотек:
[RESEARCH_V2.md](RESEARCH_V2.md) §9 / [RESEARCH_V2.ru.md](RESEARCH_V2.ru.md) §9.
Приоритеты ниже основаны на нём.

| ID | Задача | Оценка | Выбор стека (§9) |
|----|--------|--------|------------------|
| G-01 | Движок политик: allowlist зон/целей, quiet mode, read-only, подтверждение авторизации | 9/M/High | ядро без новых зависимостей |
| G-02 | MQTT-паблишер (JSON + Sparkplug B) в локальный брокер, opt-in | 8/L/Medium | `rumqttc` + `prost`, свой кодек Sparkplug |
| G-03 | OPC UA-сервер: карта тегов как адресное пространство, подпись/шифрование | 8/L/Medium | `async-opcua` (server, зафиксировать версию) |
| G-04 | Web-компаньон для просмотра проектов и трендов через локальный REST/WS API | 7/M/Low | Vue 3 + существующий API |
| G-05 | BACnet/IP-коннектор (read-only объекты, COV) — отложен | 6/L/Medium | наблюдение за `bacnet-rs` 0.3.x, feature-флаг |
| G-06 | Plugin SDK: стабильные трейты + WASM-песочница для декодеров/транспортов | 6/L/High | in-tree трейты → `extism` + `wasmtime` |

Порядок работ: **G-01 → G-04 → G-02 → G-03 → G-06 → G-05** (быстрый выигрыш и
низкий риск сначала; BACnet — последним из-за незрелости стека 0.3.x).
Расширение протоколов (DNP3, EtherNet/IP) — только после стабилизации ядра.
Ключевые ограничения: никакого pass-through, никакой телеметрии, MQTT только по
opt-in с локальным брокером по умолчанию; WASM вместо сырых dylib.

---

## 7. Приоритизация / Scoring

| Критерий | Вес | Вопрос |
|----------|-----|--------|
| Impact | 3 | Сколько пользовательских проблем закрывает? |
| Confidence | 2 | Насколько подтверждён спрос (исследования, issue, аналоги)? |
| Effort | 1 | Оценка в днях (чем меньше, тем лучше) |
| Risk | 1 | Насколько велик риск сломать ядро/безопасность? |

Примерные оценки: E-01 (9/10/6/3), E-02 (8/9/3/2), E-03 (7/8/8/3), E-04 (9/9/5/2), E-05 (6/7/12/4), D-06 (5/8/10/4). Порядок в таблицах выше уже учитывает этот score.

---

## 8. Метрики качества / KPI

| Метрика | Цель |
|---------|------|
| Write-запросов в discovery | 0 (интеграционный тест в CI) |
| Время поиска при неизвестном baud/parity | ≤ 5 мин |
| Ложные «устройства» на открытом порту | 0 |
| Покрытие протокольного слоя тестами | ≥ 80% линий |
| CI | зелёный на Windows / Linux / macOS |
| Размер инсталлятора | ≤ 10 МБ |
| Синхронизация IPC-типов | 100% (генерация) |
| Документация | EN/RU-пары для каждого раздела |

---

## 9. Риски и митигации

| Риск | Митигация |
|------|-----------|
| Декоплинг движка от Tauri — большой рефакторинг | Ввести `EventSink` тонким слоем, не переписывать логику |
| Write-функции в security-режиме | Только opt-in, двойное подтверждение, rollback, дисклеймер |
| Перегрузка OT-устройств | Quiet mode, request gap, concurrency cap, таймауты 200–800 мс |
| Дубли адресов на шине → CRC-коллизии | Подозрение на коллизию в UI, point-to-point подсказка |
| Расхождение Rust/TS типов | Генератор типов в v1.0 |
| Регрессия локализации | Словарь как единственный источник, проверка ключей в CI |

---

## 10. Вне текущего объёма

- Мобильные платформы (Tauri Android/iOS) — desktop-only UI.
- Режим Modbus-сервера/симулятора.
- Запись в процессе обнаружения.
- Интеграция с пакетами программирования ПЛК (WinCC, TIA Portal).
- Облачная синхронизация проектов и телеметрия пользователей.

---

## 11. English summary

**Status:** v1.0.0 released with Windows installers (NSIS + MSI); all six CI
and release checks are green on Windows/Linux/macOS. Next milestones:

- **v1.1 Diagnostics (next)** — FC08/07/0B/0C, read-only FC support map,
  contiguous range detection, bitfield decoding presets, CSV/XLSX reports,
  passive fingerprinting and a quiet OT mode. Exit: zero discovery writes,
  ≤60 s FC map, deterministic rate limiting.
- **v1.2 Workflow** — scan templates/schedules, multi-project workspace,
  HTML/PDF reports, autosave/crash recovery, hotkeys/accessibility.
- **v2.0 Platform** — Modbus→MQTT/OPC UA gateways, web companion, IEC 62443
  zone-aware policies, plugin architecture; BACnet deferred. Library choices
  are backed by live registry data in [RESEARCH_V2.md](RESEARCH_V2.md) §9:
  `rumqttc` + an own Sparkplug encoder, pinned `async-opcua` in server mode,
  `extism`/`wasmtime` for WASM plugins, `bacnet-rs` on a watchlist.

Priorities are derived from [RESEARCH.md](RESEARCH.md) (official specs, field
tools, community and academic sources). Hard rules stay unchanged: read-only
discovery, explicit user action, no telemetry, own protocol stack, EN/RU docs
in the same PR. KPI: zero discovery writes, <5 min unknown-baud discovery,
zero false devices on open ports, ≥80% protocol test coverage, green CI on
3 OS, installers ≤10 MB.

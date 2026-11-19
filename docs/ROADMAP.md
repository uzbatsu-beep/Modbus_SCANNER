# Дорожная карта / Roadmap

> Обоснование приоритетов и рыночный контекст: [RESEARCH.md](RESEARCH.md) /
> [RESEARCH.ru.md](RESEARCH.ru.md).
> Детальные задачи и критерии готовности выполненных этапов: [PLAN.md](PLAN.md) /
> [PLAN.ru.md](PLAN.ru.md).
> История изменений: [CHANGELOG.md](../CHANGELOG.md).

**Дата обновления:** 2026-11-19 · **Текущий релиз:** v0.1.1 · **Ветка:** `main`

---

## 1. Где мы сейчас / Current status

| Этап | Статус | Что дал |
|------|--------|---------|
| **v0.1.0** — MVP | ✅ релиз | Протокол RTU/ASCII/TCP/UDP, транспорты, движок сканера, UI, экспорт, документация |
| **v0.2** — Field Discovery | ✅ в v0.1.1 | Автосвип baud/чётности, адаптивный register scan, классификация IP, FC43, экспорт по устройству, валидация MBAP tx id, выбор ADU, журнал зондов |
| **v0.3** — Decode & Monitor | ✅ в v0.1.1 | Декодер типов и byte order, именованные теги, проекты с историей, live-мониторинг и тренды, raw TX/RX-журнал, файловые логгеры, diff сканов, RU/EN UI и светлая тема |
| **v1.0** — Ecosystem & Automation | ✅ завершён | CLI, локальный API, профили устройств, Modbus TLS, автоматизация релизов, генерация типов, тесты и CI на 3 ОС |
| **v1.1** — Diagnostics | 📋 запланирован | FC08/07/0B/0C, FC support map, автоопределение диапазонов, bitfields, XLSX, пассивный fingerprinting |
| **v1.2** — Workflow | 📋 запланирован | Шаблоны и расписание сканов, отчёты, автосохранение, hotkeys, доступность |
| **v2.0** — Platform | 💡 исследование | Шлюзы MQTT/OPC UA/REST/BACnet, web-компаньон, политики IEC 62443, плагины |

**Качество на текущий момент:** `cargo check` и `npm run tauri build` — без предупреждений; CI зелёный на Windows-msvc и Ubuntu; Windows-инсталляторы (NSIS + MSI) загружены в Release v0.1.1.

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

---

## 5. v1.2 — Workflow & Reliability (3–4 недели, приоритет P1)

- Шаблоны сканов и расписание (cron-like) с запуском через CLI/API.
- Рабочее пространство из нескольких проектов, сравнение проектов между файлами.
- Генератор отчётов HTML/PDF с графиками трендов и diff-ผล.
- Автосохранение результатов после каждого цикла, восстановление после сбоя.
- Горячие клавиши, доступность (ARIA, контраст светлой темы).
- Ротация диагностических логов самого приложения.

---

## 6. v2.0 — Platform (8–12 недель, приоритет P2, исследование)

- Шлюзы **Modbus → MQTT / OPC UA / REST API / BACnet** (соседний рынок из [RESEARCH](RESEARCH.md)).
- Веб-компаньон для просмотра проектов и трендов с родного ПК.
- Политики безопасного сканирования по зонам/кондуитам IEC 62443 (SL-1…SL-3).
- Плагинная архитектура транспортов и декодеров.
- Расширение протоколов: DNP3, EtherNet/IP (только после стабилизации Modbus-ядра).

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

**Status:** v0.1.1 released; v0.2 (field discovery) and v0.3 (decode & monitor)
are complete. Next milestones:

- **v1.0 Ecosystem** — decouple the engine from Tauri (`EventSink`), headless
  CLI, local REST/WebSocket API, device profiles, Modbus TLS (port 802),
  release automation, generated IPC types, mock-transport tests and a 3-OS CI.
- **v1.1 Diagnostics** — FC08/07/0B/0C, read-only FC support map, contiguous
  range detection, bitfield decoding presets, CSV/XLSX reports, passive
  fingerprinting and a quiet OT mode.
- **v1.2 Workflow** — scan templates/schedules, multi-project workspace,
  HTML/PDF reports, autosave/crash recovery, hotkeys/accessibility.
- **v2.0 Platform** — Modbus→MQTT/OPC UA/REST/BACnet gateways, web companion,
  IEC 62443 zone-aware policies, plugin architecture.

Priorities are derived from [RESEARCH.md](RESEARCH.md) (official specs, field
tools, community and academic sources). Hard rules stay unchanged: read-only
discovery, explicit user action, no telemetry, own protocol stack, EN/RU docs
in the same PR. KPI: zero discovery writes, <5 min unknown-baud discovery,
zero false devices on open ports, ≥80% protocol test coverage, green CI on
3 OS, installers ≤10 MB.

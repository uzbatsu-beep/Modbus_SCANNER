# Исследование платформы v2.0

[English version](RESEARCH_V2.md) · [Текущая дорожная карта](ROADMAP.md) ·
[Исследование v0/v1](RESEARCH.ru.md)

**Дата:** 2026-11-19 · **Область:** шлюзы протоколов, web-компаньон, политика безопасности OT, плагины
**Метод:** официальные организации по стандартам, GitHub, документация вендоров,
отраслевые блоги и обсуждения сообщества через поиск.

**Дополнение (§9):** срез реестров crates.io и GitHub на 2026-11-19 — основа
для конкретного выбора библиотек в бэклоге v2.0.

---

## 1. Рынок: что на самом деле делают шлюзы

| Проект / продукт | Что подтверждает |
|-------------------|------------------|
| [ThingsBoard IoT Gateway](https://github.com/thingsboard/thingsboard-gateway) (2,2k★, Apache-2.0) | Modbus, OPC UA, BACnet, MQTT, REST, ODBC, SNMP в одном шлюзе; **маппинг данных/конвертеры**, локальная буферизация, авто-reconnect, удалённая конфигурация и логи из web UI; кастомные коннекторы |
| [Apache StreamPipes](https://streampipes.apache.org) | OPC UA, MQTT, S7, Modbus → визуальные пайплайны, дашборды и датасеты |
| [EMQX Neuron](https://hysenlabs.com/projects/emqx-neuron) (LGPL-3.0) | Modbus/OPC UA/MQTT-шлюз с web UI на порту 7000 |
| [MultiBus Gateway](https://multibus.dev) | Modbus + MQTT + HTTP + OPC UA, виртуальные счётчики, **безопасная запись**, своё железо |
| [Сравнение IoT-шлюзов 2026](https://www.modbuscloud.com/knowledge-base/industrial-iot-gateway-comparison) | Критерии покупки: Modbus + BACnet + MQTT, кибербезопасность NIS2/CRA, цены в ЕС |
| [HiveMQ: мост OPC UA ↔ MQTT](https://www.hivemq.com/blog/opc-ua-mqtt-bridge-ot-protocols-industrial-data/) | Мост OT-протоколов в масштабируемую MQTT-шину — мейнстрим-архитектура |

**Вывод:** преимущество не в «очередном Modbus-клиенте», а в **трансляции + маппинге +
буферизации + удалённом управлении**, желательно с нашей read-only моделью и
локальным хранением.

## 2. Стандарты, важные для v2.0

- **OPC UA** (IEC 62541, [OPC Foundation](https://opcfoundation.org/about/opc-technologies/opc-ua/)):
  объектная информационная модель, 60+ companion specifications, client/server
  и Pub/Sub, встроенные подпись/шифрование и X.509. Карта тегов сканера почти
  без потерь превращается в адресное пространство OPC UA.
- **Eclipse Sparkplug B** ([спецификация v3.0](https://sparkplug.eclipse.org/specification/),
  [реализация Tahu](https://projects.eclipse.org/projects/iot.tahu)): задаёт
  namespace топиков MQTT, Protobuf-полезную нагрузку и состояние сессии
  (birth/death). Обычный MQTT просто переносит сообщения; Sparkplug делает их
  совместимыми со SCADA/HMI.
- **BACnet / BACnet-IP** (ASHRAE 135-2024, [bacnet.org](https://bacnet.org)):
  объектный стандарт зданий (HVAC, освещение, учёт), ISO 16484-5; BACnet/SC
  добавляет защищённую связь. Шлюзы Modbus→BACnet — отдельный востребованный
  сегмент автоматизации зданий.
- **Modbus Security** (порт 802, TLS + взаимный X.509) — реализован в E-05 и
  прямо рекомендован в [12 практиках RTA](https://www.rtautomation.com/rtas-blog/securing-modbus-networks/).

## 3. Безопасность OT (IEC 62443)

Источники: [практическое руководство по зонам и кондуитам](https://www.cagripolat.com/iec62443/en/iec-62443-zone-conduit-design-ot-practical-guide),
[MDPI про зоны и кондуиты](https://www.mdpi.com/2624-800X/6/2/52),
[12 практик RTA](https://www.rtautomation.com/rtas-blog/securing-modbus-networks/).

Правила, которые нужно заложить именно как *политики*, а не только в документацию:

1. Изолировать сеть Modbus от IT/корпоративной; подключаться только через управляемый шлюз (зона/кондуит, SL-1…SL-3).
2. Разрешать только нужные FC — мониторингу нужны FC01–FC04; запись (FC05/06/15/16) — только по явному разрешению (least privilege по NIST).
3. **Никаких pass-through-команд** — любая возможность отправить сырой код через шлюз является поверхностью атаки. Наш `send_raw_frame` — только для отладки и должен быть под контролем.
4. Удалённый доступ — через MFA/VPN и ограниченное время; напрямую к устройствам — никогда.
5. Мониторинг аномалий: новые IP-клиенты, неожиданные FC, необычные диапазоны регистров, **быстрый перебор Unit ID и аномальная частота опроса**. Наш сканер намеренно делает свип Unit ID — v2.0 обязан иметь «тихий» профиль и подтверждение авторизации, чтобы не выглядеть атакой.
6. Data diode для зон «только наблюдение» — золотой стандарт; наш путь публикации через UDP/MQTT может логически повторять однонаправленный поток.

## 4. Web-компаньон

Шлюзы выше дают web UI для удалённой конфигурации, логов и дашбордов. Естественная эволюция проекта — **local-first**: использовать уже существующий REST + WebSocket API v1.0 (`/api/v1/*`, `monitor:update`) как бэкенд небольшого web-дашборда (проекты, тренды, карточки устройств). Без облака и телеметрии по умолчанию; публикация в MQTT — только по явному opt-in.

## 5. Архитектура плагинов

[Плагины Tauri](https://v2.tauri.app/develop/plugins/) — compile-time, двухслойные (Rust + типизированный JS API): хорошо для first-party расширений, но не для устанавливаемых пользователями runtime-плагинов. ABI `dyn Trait` в Rust нестабилен между версиями компилятора, поэтому runtime-плагины требуют одного из:

- версионированный C ABI (`abi_stable`) — высокие затраты на обслуживание;
- **WASM-песочница** (`wasmtime`/`extism`) — безопаснее, переносима, чёткие границы возможностей; рекомендуется для community-декодеров/транспортов;
- оставить протокольные модули как in-tree feature-крейты со стабильными внутренними трейтами (`Transport`, `ScanStrategy`, `EventSink`) — наше разделение в E-01 уже создало правильные швы.

Рекомендация: **сначала compile-модули, потом WASM-песочница**, никакой загрузки сырых dylib.

## 6. Приоритетный бэклог v2.0 (предложение)

| ID | Задача | Impact | Effort | Risk | Зависимости |
|----|--------|--------|--------|------|-------------|
| G-01 | Движок политик: allowlist зон/целей, quiet mode, read-only, подтверждение авторизации | 9 | M | High | ядро v1.0 |
| G-02 | MQTT-паблишер (plain JSON + Sparkplug B protobuf) в локальный брокер, opt-in | 8 | L | Medium | G-01 |
| G-03 | OPC UA-сервер: карта тегов как адресное пространство, подпись/шифрование | 8 | L | Medium | теги v1.0 |
| G-04 | Web-компаньон на локальном API | 7 | M | Low | REST/WS v1.0 |
| G-05 | BACnet/IP-коннектор (read-only объекты, COV) | 6 | L | Medium | G-01 |
| G-06 | Plugin SDK: стабильные трейты + WASM-песочница для декодеров/транспортов | 6 | L | High | G-01, E-01 |

## 7. Риски / митигации

| Риск | Митигация |
|------|-----------|
| Шлюз станет pass-through-вектором | G-01: движок политик, raw-кадры по умолчанию выключены |
| Утечка OT-данных в облако через MQTT | opt-in, локальный брокер по умолчанию, allowlist топиков, без телеметрии |
| Сложность управления сертификатами OPC UA | старт с server mode и политиками None/Sign, описание workflow сертификатов |
| Размер BACnet-стека | вынести за feature-флаг, рассмотреть зрелый крейт |
| Ограничения WASM-песочницы | плагины только преобразуют данные; транспорты остаются в ядре |

## 9. Выбор технологий — дополнение (2026-11-19)

Первая часть исследования отвечала на вопрос *что* должен делать шлюз v2.0.
Это дополнение отвечает на вопрос *из чего мы это строим*, опираясь на живые
данные реестров, чтобы бэклог в [ROADMAP.md](ROADMAP.md) базировался на
проверяемых фактах о поддержке и лицензиях, а не на интуиции.

### 9.1 Кандидаты (срез 2026-11-19)

| Область | Крейт / проект | Версия | Последний релиз | Лицензия | Загрузок за месяц | Вердикт |
|---------|----------------|--------|-----------------|----------|-------------------|---------|
| OPC UA | [`async-opcua`](https://github.com/freeopcua/async-opcua) | 0.19.0 | 2026-07-18 | MPL-2.0 | 121k | **Выбран** — активный async client+server, отдельный feature `server` |
| OPC UA (устаревший) | [`opcua`](https://github.com/locka99/opcua) | 0.12.0 | 2024-01-01 | MPL-2.0 | 27k | Отвергнут — релизов нет ~2 года (репозиторий последний раз 2025-04) |
| MQTT | [`rumqttc`](https://github.com/bytebeamio/rumqtt) | 0.25.1 | 2025-11-21 | Apache-2.0 | 2,59M | **Выбран** — де-факто стандарт, rustls и WebSocket из коробки |
| MQTT (альтернатива) | [`paho-mqtt`](https://github.com/eclipse/paho.mqtt.rust) | 0.14.0 | 2026-03-26 | EPL-2.0 | 392k | Резерв — условия EPL-2.0 по патентам требуют проверки |
| BACnet | [`bacnet-rs`](https://github.com/bacnet-rs/bacnet-rs) | 0.3.2 | 2026-09-02 | MIT OR Apache-2.0 | 8k | **Наблюдение** — поддерживается, но pre-1.0 и маленькое сообщество (~28★) |
| BACnet (устаревший) | [`bacnet`](https://github.com/BiancoRoyal/bacnet-rust) | 0.1.0 | 2018-05-14 | MPL-2.0 | 23 | Отвергнут — заброшен |
| Sparkplug B | зрелого Rust-крейта нет | — | — | — | — | **Свой кодек** на [`prost`](https://github.com/tokio-rs/prost) 0.14.6; в GitHub найдены только прототипы на 0–10★ |
| WASM-плагины | [`extism`](https://github.com/extism/extism) | 1.41.0 | 2026-10-09 | BSD-3-Clause | 239k | **Выбран** для G-06 (SDK с capability-моделью) |
| WASM-рантайм | [`wasmtime`](https://github.com/bytecodealliance/wasmtime) | 51.0.0-rc | 2026-11-19 | Apache-2.0 WITH LLVM-exception | 13,4M | **Выбран**, но фиксируем *стабильный* релиз, не RC |
| Modbus (только справка) | [`tokio-modbus`](https://github.com/slowtec/tokio-modbus) | 0.17.0 | 2025-10-22 | MIT OR Apache-2.0 | 477k | Не используется — правило собственного стека сохраняется |

### 9.2 Решения по бэклогу

| ID | Решение | Обоснование |
|----|---------|-------------|
| G-01 | Чистая логика ядра, без новых зависимостей | Движок политик — это логика, а не I/O; должен тестироваться без железа |
| G-02 | `rumqttc` + `prost`; локальный брокер, публикация по opt-in | Зрелой Rust-реализации Sparkplug B нет: сначала JSON, затем Protobuf, свой тонкий кодек |
| G-03 | `async-opcua` в режиме сервера, только feature `server` | Активный апстрим; старт с политик None/Sign, описать workflow X.509 |
| G-04 | Vue 3 + существующий REST/WS API, без нового бэкенда | Минимальный риск, переиспользуем контракт v1.0 |
| G-05 | Отложить; следить за `bacnet-rs` до 1.0 или появления сообщества | Pre-1.0 стек на отказоустойчивом пути — риск цепочки поставок |
| G-06 | Сначала стабильные in-tree трейты (швы E-01), затем `extism` + зафиксированный `wasmtime` | Никакого сырого dylib ABI; WASM-возможности совпадают с моделью «плагин только преобразует данные» |

### 9.3 Совместимость лицензий с GPL-3.0

MPL-2.0 (`async-opcua`), Apache-2.0 (`rumqttc`, `wasmtime`, `prost`), MIT и
BSD-3-Clause (`extism`) совместимы с GPL-3.0: они не накладывают дополнительных
ограничений на комбинированную работу, а copyleft MPL-2.0 действует на уровне
файлов. Исключение — EPL-2.0 (`paho-mqtt`): его условия о патентном возмездии
и исключениях нужно явно проверять перед внедрением, поэтому предпочтительнее
`rumqttc`. Плагины должны поставлять собственные уведомления о лицензии;
WASM-песочница не является исключением для линкования.

### 9.4 Новые риски

| Риск | Митигация |
|------|-----------|
| OPC UA-крейт имеет версию 0.x и может сломать API в минорном релизе | Фиксировать точную версию и изолировать за своим адаптером |
| `bacnet-rs` — 0.3.x с маленьким сообществом | Держать G-05 за feature-флагом; вернуться после 1.0 или сделать минимальный read-only BACnet/IP самим |
| Зрелой Rust-реализации Sparkplug B нет | Свой тонкий Protobuf-кодек, проверенный тестовыми векторами Tahu; никаких самодельных форматов |
| wasmtime часто публикует RC | Фиксировать стабильную версию в `Cargo.lock`, обновлять осознанно, никогда не `*` |

### 9.5 Источники дополнения

15. https://crates.io/api/v1/crates/async-opcua · https://github.com/freeopcua/async-opcua
16. https://crates.io/api/v1/crates/opcua
17. https://crates.io/api/v1/crates/rumqttc
18. https://crates.io/api/v1/crates/paho-mqtt
19. https://crates.io/api/v1/crates/bacnet-rs · https://github.com/bacnet-rs/bacnet-rs
20. https://crates.io/api/v1/crates/bacnet
21. https://api.github.com/search/repositories?q=sparkplug+rust · https://sparkplug.eclipse.org/specification/
22. https://crates.io/api/v1/crates/prost
23. https://crates.io/api/v1/crates/extism · https://crates.io/api/v1/crates/wasmtime
24. https://crates.io/api/v1/crates/tokio-modbus

---

## 10. Источники

1. https://github.com/thingsboard/thingsboard-gateway
2. https://streampipes.apache.org
3. https://multibus.dev
4. https://www.modbuscloud.com/knowledge-base/industrial-iot-gateway-comparison
5. https://www.hivemq.com/blog/opc-ua-mqtt-bridge-ot-protocols-industrial-data/
6. https://sparkplug.eclipse.org/specification/
7. https://projects.eclipse.org/projects/iot.tahu
8. https://opcfoundation.org/about/opc-technologies/opc-ua/
9. https://reference.opcfoundation.org/specs/OPC-10000-2/4.5
10. https://bacnet.org · https://data.ashrae.org/BACnet/
11. https://www.cagripolat.com/iec62443/en/iec-62443-zone-conduit-design-ot-practical-guide
12. https://www.mdpi.com/2624-800X/6/2/52
13. https://www.rtautomation.com/rtas-blog/securing-modbus-networks/
14. https://v2.tauri.app/develop/plugins/

# Исследование платформы v2.0

[English version](RESEARCH_V2.md) · [Текущая дорожная карта](ROADMAP.md) ·
[Исследование v0/v1](RESEARCH.ru.md)

**Дата:** 2026-11-19 · **Область:** шлюзы протоколов, web-компаньон, политика безопасности OT, плагины
**Метод:** официальные организации по стандартам, GitHub, документация вендоров,
отраслевые блоги и обсуждения сообщества через поиск.

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

- версионированный C ABI (`abi_stable`) — tinggi обслуживание;
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

## 8. Источники

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

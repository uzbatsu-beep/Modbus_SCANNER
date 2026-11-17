# Market & community research / Исследование рынка и сообщества

**Дата исследования:** 2026-11-18. **Метод:** доступные публичные источники —
официальный сайт Modbus Organization, документация и страницы коммерческих и
open-source инструментов, GitHub-репозитории, академические публикации (IEEE,
Springer, MDPI, Computers & Security), форумы/Reddit (через поисковые сниппеты),
отраслевые гайды и стандарты (ISA/IEC 62443, NIS2).

> Ограничение: в ходе исследования `websearch` был недоступен, часть страниц
> (Reddit, ScienceDirect, PDF-спецификации) блокировала прямой доступ. Для них
> использованы поисковые сниппеты и альтернативные зеркала; это отмечено ниже.

---

## 1. Что говорят официальные источники / Official sources

Источник: [Modbus Organization — Specifications and Implementation Guides](https://modbus.org/modbus-specifications)

| Документ | Суть для продукта |
|----------|-------------------|
| Modbus Application Protocol V1.1b3 | Базовый набор FC, адресация, исключения; наш протокольный слой уже следует ему |
| Modbus Serial Line Protocol and Implementation Guide V1.02 | Рекомендуется для новых реализаций (наша RTU/ASCII-реализация соответствует) |
| Modbus Messaging Implementation Guide v1.0b (TCP/IP) | MBAP, клиент/сервер/шлюз, объектная модель |
| **Modbus Security Protocol (TLS, порт 802)** | Перспектива: аутентификация и целостность через TLS + X.509v3; пока не реализовано ни у нас, ни в большинстве сканеров |

Вывод: протокольная база у проекта правильная; ближайший официальный вектор
роста — **Modbus TLS/TCP 802** и поддержка identification/diagnostic FC.

---

## 2. Что востребовано в полевых инструментах / What field tools do

### 2.1 Modbus Logic Scanner — эталон read-only сканера
Источник: [docs.modbuslogic.com](https://www.modbuslogic.com/docs/modbus-scanner/)

Ключевые возможности, которые напрямую задают «планку» продукта:

- **Discover**: CIDR/IP-диапазон, port, timeout, concurrency; классификация
  `valid Modbus response` / `port open but no Modbus` / `refused` / `timeout` /
  `unreachable`. Открытый порт 502 **не** считается устройством.
- RTU: свип unit ID на одном порту; для TCP — discovery unit ID за шлюзом
  (по умолчанию 1–10).
- **Register scan** с тремя стратегиями: `Quick` (блоки, failed block → 1/5),
  `Balanced` (100 → 20 → 5, затем уточнение границ), `Deep` (до одиночных
  регистров). Timeouts не делятся (дорого), 3 timeout подряд прерывают,
  exception 01 прерывает сразу.
- Исключение `02 Illegal Data Address` трактуется как **норма**: устройство
  отвечает, просто адреса нет (то же самое описано в [ModbusCloud KB](https://www.modbuscloud.com/knowledge-base/modbus-network-scan)).
- Адреса в 4 нотациях: zero-based, one-based, reference, hex.
- Декодирование INT16/UINT16/INT32/UINT32/FLOAT32/ASCII + **все byte orders**;
  подсказки вида «possible float32 (ABCD)» (никогда не выдаётся как факт).
- Именованные регистры: тип, byte order, **scaling** `(raw × mul ÷ div) + offset`,
  единицы, описание → `io-tags.json`.
- **Live Monitor**: polling 250 мс–10 с, min/max, тренд 1 мин–1 ч; Brooks-эффект:
  мониторинг — финальная проверка правильности декодирования.
- Экспорты: `io-tags.json`, CSV/Excel, raw register CSV, simulator CSV; проекты
  и история сканов.
- **Read-only by design**: только FC01–04 и read-only FC43/14; в интеграционных
  тестах Assert: mock-устройство получает **ноль** write-запросов. Ничего не
  запускается в фоне, only 127.0.0.1, без телеметрии.
- Прямое предупреждение: даже чтение создаёт нагрузку на промышленные устройства.

### 2.2 Modbus Connect — мониторинг и логирование
Источник: [modbusconnect.com/features](https://modbusconnect.com/features)

- Discovery: CIDR, **quiet mode для production** и fast mode с конкуренцией,
  ID 1–247, извлечение **vendor/product code/model**, bulk add.
- Реальный мониторинг: подсветка изменений, произвольные метки, dec/hex/bin,
  пауза сессии без потери данных, поддержка 1-based offset.
- Графики до 12 рядов, zoom/pan, pop-out окна, экспорт данных графика в CSV.
- **Protocol-level debugging**: raw TX/RX hex с таймстемпами, FC и ошибки.
- File logging: CSV/JSON/TAB, ротация daily/session/size, несколько логгеров.

### 2.3 Другие инструменты (сводка по поисковым выдачам)

| Инструмент | Что отличает | Источник |
|------------|--------------|----------|
| CAS Modbus Scanner | Binary/HEX/Uint16/Int16/Uint32/Int32/Float32; поиск устройств без известного адреса | [Chipkin](https://store.chipkin.com/products/tools/cas-modbus-explorer) |
| modbus-rtu-tool (GitHub) | Полный bus scan: **все baud/parity/stop + ID 1–247**, register map scan, монитор 10×N, write panel, i18n RU/EN, single-file exe | [GitHub](https://github.com/olegchumakov/modbus-rtu-tool) |
| modbus-probe (GitHub) | Security-аудит: unit enum, fuzz FC 1–127, write-тест с rollback, FC43 fingerprinting, **PCAP capture**, multi-thread, JSON/CSV, severity, `--monitor`, `--discover` CIDR; обоснование — IEC 62443 / NIS2 | [GitHub](https://github.com/isecwire/modbus-probe) |
| Nmap `modbus-discover` | Перебор unit ID + чтение vendor/product через FC2B/MEI0E | [Nmap NSE](https://nmap.org/nsedoc/scripts/modbus-discover.html) |
| ModbusManager / ModbusSeeker | Авто-определение baud/parity и slave ID как главная ценность («нет документации») | [modbusmanager.com](https://modbusmanager.com/modbus-scanner.html), [modbusseeker.com](https://modbusseeker.com) |
| ModbusLens | Бесплатный TCP/RTU сканер + клиент + сервер, live tags, тренды | [ModbusLens](https://craftparking.github.io/ModbusLens/) |
| modbus-map-maker | Превращает таблицы регистров в валидированные spec (endianness-aware) + код | [GitHub](https://github.com/casalifrancesco98/modbus-map-maker) |
| modpoll / pymodbus / QModMaster / OpenModScan | CLI-скриптинг, кросс-платформенность, базовый GUI | [ModbusCloud KB](https://www.modbuscloud.com/knowledge-base/modbus-network-scan) |

### 2.4 Что волнует сообщество / Community signals

- [Reddit r/PLC «Any helpful Modbus diagnostic tools out there?»](https://www.reddit.com/r/PLC/comments/f2728e/any_helpful_modbus_diagnostic_tools_out_there/):
  в сниппете — боль «save you from decoding the data», **node address** и
  **one-based vs zero-based addressing**. То есть даже после обнаружения
  пользователи застревают на интерпретации адресов и данных.
- [Chipkin: Modbus Device Discovery Guide](https://docs.chipkin.com/articles/modbus-device-discovery-and-network-scanning-guide/)
  и [ModbusCloud KB](https://www.modbuscloud.com/knowledge-base/modbus-network-scan):
  * сначала свип speed/framing (9600 8N1 → 2400 8N1 → 19200 8N1 → 8E1), потом адреса;
  * ловушки: **дубли адресов → CRC-коллизии**, broadcast 0, отсутствие терминатора;
  * на шине не должно быть двух master — сканирование при работающем PLC даёт мусор;
  * полный snapshot не заменяет непрерывный мониторинг;
  * поиск по nmap: `-p 502 --open`, `modbus-discover`.
- Итог: «просто найти адрес» — уже commodity. Ценность — **autodetect + корректная
  интерпретация + безопасность + экспорт в работу**.

---

## 3. Что говорят научные работы / Academic research

| Работа | Вывод для продукта |
|--------|--------------------|
| [Passive Fingerprinting of ICS Devices Using Aggregates of Modbus Traffic (IEEE, 2025)](https://ieeexplore.ieee.org/document/11471618/) | «Active scanning may have adverse effects on an industrial network» → нужен **passive/benign режим** и минимизация активных запросов |
| [Remote field device fingerprinting using device-specific Modbus information (IEEE, 2017)](https://ieeexplore.ieee.org/document/7870006) | Возможна идентификация модели/вендора по Modbus-ответам |
| [On the Feasibility of Device Fingerprinting in ICS, LNCS 8328 (Springer, 2014)](https://link.springer.com/chapter/10.1007/978-3-319-03964-0_14) | Обзор и рекомендации по надёжному fingerprinting в ICS |
| [Development of Fingerprint Identification Based on Device Flow (MDPI Applied Sciences, 2023)](https://www.mdpi.com/2076-3417/13/2/731) | Периодичность трафика как признак устройства → база для будущего passive-режима |
| [MOSTO: toolkit for security auditing of ICS devices (Computers & Security, 2023)](https://www.sciencedirect.com/science/article/pii/S0167404823002833) | GPL-инструмент аудита, который «не вмешивается в ICS workflow» — прямо совпадает с нашей read-only позицией |
| [ANDVI: Automated Network Device and Vulnerability Identification](https://par.nsf.gov/servlets/purl/10495895) | Извлечение Modbus ADU из TCP-пакетов для идентификации |
| [Read Device Identification FC43/MEI14](https://www.merobix.com/blog/what-is-modbus-function-code-43-device-identification) | Basic ID: VendorName, ProductCode, Revision — минимум; опциональная часть спецификации |

Вывод: академический консенсус — активное сканирование OT опасно, поэтому
безопасный reading-only сканер с «тихим» режимом и пассивным fingerprinting —
это не просто UX, а конкурентное преимущество и соответствие практикам.

---

## 4. Стандарты и регуляторика / Standards

- [ISA/IEC 62443 series](https://www.isa.org/standards-and-publications/isa-standards/isa-iec-62443-series-of-standards) —
  зоны/кондуиты, уровни безопасности SL1–SL4; требует контроля доступа, которого
  у Modbus нет.
- EU **NIS2** и **Cyber Resilience Act** (обязанность сообщать об уязвимостях
  с июня 2026, см. [ModulesLink](https://www.moduleslink.com/article-iec62443-industrial-cybersecurity.html)) —
  интерес к аудиту OT растёт; security-режим сканера востребован, но должен быть
  opt-in и с авторизацией.
- Практический вывод: **не делать write-тесты в основном потоке**; если делать —
  отдельный режим с подтверждением, автоматическим rollback и предупреждением
  об ответственности.

---

## 5. Ранжирование востребованности / Demand ranking

| # | Возможность | Частота в источниках | Наш MVP |
|---|-------------|----------------------|---------|
| 1 | Авто-подбор baud/parity/stop + свип slave ID | Очень высокая (почти каждый полевой гайд) | Частично (ID есть, speed sweep нет) |
| 2 | Умное сканирование карты регистров (блоки/сплит/exception-aware) | Очень высокая | Частично (линейный register_plan) |
| 3 | Декодирование типов, byte/word order, scaling, именование | Очень высокая | Нет (только raw hex/dec/bin) |
| 4 | Read-only by design, quiet mode, лимиты, авторизация | Высокая (community + стандарты) | Частично (read-only есть, quiet нет) |
| 5 | Идентификация устройства FC43/14 + vendor/product | Высокая | Нет |
| 6 | Live-мониторинг, тренды, подсветка изменений, логи с ротацией | Высокая | Нет |
| 7 | Экспорт tag-map/проектов/истории/сравнение | Средне-высокая | Есть базовый CSV/JSON/XML |
| 8 | IP discovery с классификацией «порт открыт, но не Modbus», concurrency | Средне-высокая | Частично (sequential) |
| 9 | Raw TX/RX лог, тайминги, PCAP, FC support map/fuzz | Средняя (security-ниша) | Нет |
| 10 | CLI/headless/REST, скриптинг, интеграции | Средняя | Нет |
| 11 | Мультипротокол (MQTT/OPC UA/REST gateway) | Средняя (соседний рынок) | Вне скоупа |
| 12 | Локализация, single-file дистрибутив, горячие клавиши | Средняя | Частично (RU/EN доки, UI смешанный) |

---

## 6. Ключевой инсайт / Key insight

> Рынок «сканеров адресов» насыщен (modpoll, CAS, QModMaster, nmap). Недостаток —
> на стыке **обнаружения и понимания**: автоматическая настройка скорости,
> безопасное сканирование карты регистров, корректное декодирование float/endianness,
> идентификация устройства и экспорт готовой карты тегов. Именно здесь наш
> собственный протокольный стек даёт контроль над таймингами и поведением —
> это наше отличие от обёрток над pymodbus.

---

## 7. Список источников / Sources

1. https://modbus.org/modbus-specifications
2. https://www.modbuslogic.com/docs/modbus-scanner/
3. https://modbusconnect.com/features
4. https://www.modbuscloud.com/knowledge-base/modbus-network-scan
5. https://docs.chipkin.com/articles/modbus-device-discovery-and-network-scanning-guide/
6. https://github.com/olegchumakov/modbus-rtu-tool
7. https://github.com/isecwire/modbus-probe
8. https://www.reddit.com/r/PLC/comments/f2728e/any_helpful_modbus_diagnostic_tools_out_there/
9. https://ieeexplore.ieee.org/document/11471618/
10. https://ieeexplore.ieee.org/document/7870006
11. https://link.springer.com/chapter/10.1007/978-3-319-03964-0_14
12. https://www.mdpi.com/2076-3417/13/2/731
13. https://www.sciencedirect.com/science/article/pii/S0167404823002833
14. https://par.nsf.gov/servlets/purl/10495895
15. https://www.merobix.com/blog/what-is-modbus-function-code-43-device-identification
16. https://www.isa.org/standards-and-publications/isa-standards/isa-iec-62443-series-of-standards
17. https://nmap.org/nsedoc/scripts/modbus-discover.html
18. https://github.com/black-a85/ModbusTool
19. https://craftparking.github.io/ModbusLens/
20. https://github.com/casalifrancesco98/modbus-map-maker

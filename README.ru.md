# Modbus Scanner

[![CI](https://github.com/uzbatsu-beep/Modbus_SCANNER/actions/workflows/ci.yml/badge.svg)](https://github.com/uzbatsu-beep/Modbus_SCANNER/actions/workflows/ci.yml)

[English version](README.md)

Кроссплатформенный десктопный сканер Modbus-устройств.
Бэкенд — Rust + Tauri 2, фронтенд — Vue 3 + TypeScript. Стек Modbus реализован
собственными силами (без сторонних Modbus-кратов), чтобы полностью контролировать
фрейминг, тайминги и поведение сканера.

## Возможности

| Транспорт | Что сканируется |
|-----------|-----------------|
| **Modbus RTU** | Slave ID 0–247 на последовательном порту, скорости 1200…115200, форматы 8N1/8E1/8O1/7N1 и др. |
| **Modbus ASCII** | Slave ID 0–247, hex-фрейминг с LRC, разделители `:` и CRLF. |
| **Modbus TCP** | Диапазоны IP или CIDR-подсети, порт 502 по умолчанию, Unit ID 1/255 (маршрутизация через шлюз). |
| **Modbus UDP** | Диапазоны IP, бессоединённый зонд (нет ответа = нет устройства), ADU в стиле RTU. |

**Функции:** FC01 Read Coils, FC02 Read Discrete Inputs, FC03 Read Holding
Registers, FC04 Read Input Registers, запись FC05/06/0F/10/16/17 (в ручной
консоли и при чтении карты регистров).

**Инструменты:**
- Обнаружение устройств с настраиваемыми таймаутом, ретраями и FC зондов
- Живой прогресс и события обнаружения через Tauri events
- Чтение карты регистров найденных устройств (hex / dec / bin)
- Ручная консоль запросов: чтение, запись, отправка raw-фрейма
- Перечисление COM-портов (VID/PID, производитель, устройство) и тест порта
- Экспорт результатов в CSV / JSON / XML
- Сохранение настроек в `config.json` каталога конфигурации Tauri

## Структура проекта

```
.
├── src/                       # Фронтенд Vue 3 + TypeScript
│   ├── api/                   # Типизированные обёртки invoke/events Tauri
│   ├── components/            # layout / формы сканирования / результаты
│   ├── stores/                # Pinia: scanner, devices, serial, settings
│   ├── types/ipc.ts           # TS-зеркало IPC-контракта Rust
│   └── views/                 # Scan, Results, Manual, Settings
├── src-tauri/                 # Бэкенд Rust (Tauri 2)
│   └── src/
│       ├── modbus/            # Протокол: PDU/ADU, RTU (CRC-16), ASCII (LRC)
│       ├── transport/         # Async I/O: serial (tokio-serial), TCP, UDP
│       ├── scanner/           # Движок, стратегии, типы и результаты
│       ├── commands/          # Обработчики #[tauri::command]
│       └── state.rs           # Состояние приложения + конфигурация
├── docs/                      # Архитектура, протокол, сканирование, API, разработка
├── package.json
└── README.md / README.ru.md
```

## Документация

- [Архитектура](docs/ARCHITECTURE.ru.md) — слои, асинхронная модель, потоки данных
- [Протокол Modbus](docs/MODBUS.ru.md) — фрейминг, функции, исключения, тайминги
- [Сканирование](docs/SCANNING.ru.md) — стратегии, таймауты, безопасность
- [IPC API](docs/API.ru.md) — команды и события со схемами полезных данных
- [Разработка](docs/DEVELOPMENT.ru.md) — окружение, сборка, тесты, соглашения
- [Исследование](docs/RESEARCH.ru.md) — рынок, сообщество, наука и источники
- [Исследование платформы v2.0](docs/RESEARCH_V2.ru.md) — шлюзы, web-компаньон, IEC 62443, плагины
- [План разработки](docs/PLAN.ru.md) — приоритетные этапы v0.2 → v1.0
- [Аудит](docs/AUDIT.md) — git, надёжность, документация и прозрачность
- [Дорожная карта](docs/ROADMAP.md) — текущий статус и планы
- [Безопасность](docs/SECURITY.md) — риски записи в неизвестные устройства

## Быстрый старт

Нужны: **Node.js 18+**, **Rust 1.77+**, зависимости Tauri 2
([официальный гайд](https://v2.tauri.app/start/prerequisites/)). На Windows
требуется WebView2 Runtime.

```bash
npm install
npm run tauri dev        # запуск Vite + окна Tauri
```

### Headless CLI

Тот же движок работает без GUI: `scan` печатает JSON-отчёт (или пишет его в файл
через `--output`):

```bash
# последовательная шина, авто-свип скорости/чётности
src-tauri/target/release/modbus-scanner.exe scan --port COM3 --auto --output report.json

# подсеть Modbus TCP
src-tauri/target/release/modbus-scanner.exe scan --ip 192.168.1.0/24 --tcp-port 502

# RTU поверх TCP / UDP, свои тайминги
src-tauri/target/release/modbus-scanner.exe scan --ip 10.0.0.5 --rtu-over-tcp --timeout-ms 300
src-tauri/target/release/modbus-scanner.exe scan --ip 10.0.0.5 --udp --udp-mbap

# Modbus Security (TLS, порт 802; опционально взаимная аутентификация)
src-tauri/target/release/modbus-scanner.exe scan --ip 10.0.0.5 --tls --tls-cert client.pem --tls-key client.key
```

Полный список: `modbus-scanner scan --help`. Без подкоманды запускается десктопное
приложение. CLI использует `EventSink` вместо Tauri, поэтому логика сканирования
полностью общая.

Локальный REST + WebSocket API (только loopback):

```bash
modbus-scanner.exe serve --port 8787
curl http://127.0.0.1:8787/health
curl http://127.0.0.1:8787/api/v1/results
```

Список эндпоинтов и поток событий — в [docs/API.ru.md](docs/API.ru.md).

Production-сборка:

```bash
npm run tauri build      # инсталляторы в src-tauri/target/release/bundle/
```

Полезные команды:

```bash
npm run typecheck        # vue-tsc
cargo check              # в каталоге src-tauri/
cargo test --lib         # юнит-тесты протокола
```

## Статус

**В разработке:** кандидат v1.0.1 с исправлениями фрейминга TCP/RTU, проверкой диапазонов и защитой ручной записи. Версия будет опубликована после PR-проверки и полевых тестов.


Протокольный слой, транспорты, движок сканера и UI реализованы, проект
выпущен как **v1.0.0** (headless CLI, локальный REST/WS API, профили устройств,
Modbus TLS, автоматизация релизов, генерация IPC-типов, mock-тесты и CI на 3 ОС).
Ранние релизы `v0.1.0`/`v0.1.1` включали MVP и наборы возможностей v0.2/v0.3
(обнаружение и декодирование/мониторинг) — см. [ROADMAP](docs/ROADMAP.md) и
[PLAN](docs/PLAN.ru.md).

## Лицензия

[GNU GPL v3.0](LICENSE) — свободное ПО с копилефтом. Полный текст — в [LICENSE](LICENSE).

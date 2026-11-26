# Архитектура

[English version](ARCHITECTURE.md)

## Общая схема

```
┌────────────────────────────── Фронтенд Vue 3 ──────────────────────────────┐
│ Views (Scan / Results / Manual / Settings)                                 │
│   └─ Pinia stores (scanner, devices, serial, settings)                     │
│        └─ src/api/mod.ts  ── invoke() / listen() ──────────────┐           │
└────────────────────────────────────────────────────────────────┼───────────┘
                                  Tauri IPC (JSON)              │
┌────────────────────────────────────────────────────────────────┼───────────┐
│ Бэкенд Rust (src-tauri)                                        ▼           │
│  ┌────────────────┐  ┌────────────────┐  ┌─────────────────────────────┐  │
│  │ commands/      │  │ state.rs       │  │ scanner/                    │  │
│  │ #[tauri::cmd]  │──▶ AppState       │◀─│ ScannerHandle (tokio task)  │  │
│  └───────┬────────┘  │ config Mutex   │  │ strategies / types / engine │  │
│          │           └────────────────┘  └──────────┬──────────────────┘  │
│          │                                          │ events              │
│          ▼                                          ▼ scan:event/progress │
│  ┌────────────────┐  ┌────────────────┐                                    │
│  │ modbus/        │◀─│ transport/     │  serial (RTU/ASCII), TCP, UDP      │
│  │ PDU/ADU/codecs │  │ async I/O      │                                    │
│  └────────────────┘  └────────────────┘                                    │
└────────────────────────────────────────────────────────────────────────────┘
```

## Слои

### 1. Протокольный слой — `src-tauri/src/modbus/`

Чистая логика протокола, без I/O:

| Модуль | Ответственность |
|--------|-----------------|
| `common.rs` | `UnitId`, `Address`, `Quantity`, `TransportKind`, `SerialMode`, `ModbusError` со всеми стандартными кодами исключений |
| `frame.rs` | `FunctionCode`, `Pdu` (Request/Response/Exception), варианты `Adu`, сборщики запросов и парсеры ответов |
| `rtu.rs` | RTU ADU: `encode`/`decode` с CRC-16 (полином `0xA001`, инициализация `0xFFFF`, младший байт первым) |
| `ascii.rs` | ASCII ADU: `':' + hex(payload + LRC) + CRLF`, 8-битный LRC (дополнение до 2 по модулю 256) |

MBAP-фрейминг TCP сейчас живёт в транспорте (`transport/tcp.rs`) и в сборщиках
ADU движка (`build_adu`). Перенос в `modbus/tcp.rs` — в ROADMAP.

### 2. Транспортный слой — `src-tauri/src/transport/`

Единый асинхронный трейт `Transport` (через `async-trait`):

```rust
#[async_trait]
pub trait Transport: Send {
    fn name(&self) -> String;
    async fn reset(&mut self) -> TransportResult<()>;      // сброс старых данных RX
    async fn close(&mut self) -> TransportResult<()>;
    async fn send_raw(&mut self, adu: &[u8]) -> TransportResult<()>;
    async fn recv_raw(&mut self, hint: usize) -> TransportResult<Vec<u8>>;
    async fn transact(&mut self, adu, timeout, hint) -> TransportResult<Vec<u8>>;
}
```

Реализации:

- `SerialTransport` — `tokio-serial`, настраиваемые скорость/чётность/биты/стоп-биты,
  очистка RX, межкадровый интервал t3.5. RTU определяет конец кадра по тишине
  (5 мс), ASCII читает до CRLF.
- `TcpTransport` — `tokio::net::TcpStream`, `TCP_NODELAY`, фрейминг по длине MBAP,
  генерация transaction id. Умеет отправлять готовые MBAP-кадры как есть
  ("RTU over TCP", шлюзы).
- `UdpTransport` — `tokio::net::UdpSocket`, одна датаграмма = один ADU; отсутствие
  датаграммы трактуется как `Timeout`.

### 3. Слой сканера — `src-tauri/src/scanner/`

- `types.rs` — **IPC-контракт**: `ScanTarget`, `ScanParams`, `ProbeResult`,
  `DeviceRecord`, `RegisterValue`, `ScanStatus`, `ScanProgress`, `ScanEvent`,
  `ScanResults`/`ScanStats`. Все типы derivatives `serde`, а `src/types/ipc.ts` —
  их точное TypeScript-зеркало.
- `strategies.rs` — трейт `ScanStrategy` (`Send`), производящий `WorkItem`:
  - `UnitIdStrategy`: диапазон unit id на одном COM-порту, зонды по `probe_functions`;
  - `IpRangeStrategy`: диапазон IPv4 / CIDR, unit id 1 и 255 по умолчанию;
  - план регистров → дополнительные `RegisterRead` после обнаружения устройства.
- `engine.rs` — `ScannerHandle` (лежит в состоянии Tauri) и задача tokio:
  - управление через `mpsc<ControlMsg>` (Pause/Resume/Stop) и `watch<bool>` для паузы;
  - статус через `watch<ScanStatus>`;
  - результаты в `Arc<RwLock<ScanResults>>` (parking_lot);
  - один последовательный транспорт на всю шину (шину нельзя мультиплексировать);
  - TCP/UDP-транспорты открываются на каждый запрос;
  - события фронтенду: `scan:progress`, `scan:event`.

### 4. Слой команд — `src-tauri/src/commands/`

Тонкие функции `#[tauri::command]`, возвращающие `Result<T, String>`. Они
преобразуют IPC-вызовы в операции сканера/транспорта. Файлы: `system`, `serial`,
`scanner`, `modbus_cmd`, `config`. Все команды регистрируются в `lib.rs` через
`tauri::generate_handler!`.

### 5. Фронтенд — `src/`

- `views/` — Scan, Results, Profiles, Diff, Traffic, Manual, Settings.
- `stores/` — Pinia: состояние/журнал сканера, устройства, порты, настройки.
- `api/mod.ts` — типизированные обёртки `invoke` и `listen` из `@tauri-apps/api`.
- `types/ipc.ts` — зеркало Rust-типов; синхронизируется вручную (генератор схем
  в ROADMAP).

## Асинхронность и конкуренция

- Tauri хранит состояние приложения (`AppState`: `Arc<ScannerHandle>`, `MonitorHandle`,
  `TrafficLogHandle`, `LoggerHandle`, дескриптор локального API и `Config` в мьютексе).
- `start_scan` планирует одну задачу tokio; IPC-вызов возвращается сразу.
- Последовательный scan **обязан** быть последовательным — общая шина не
  поддерживает параллельные запросы.
- IP-сканирование сейчас тоже выполняется последовательно; параметр
  `ip_concurrency` зарезервирован для планируемого пула воркеров (отдельный
  транспорт на цель).
- Write-guard'ы `parking_lot::RwLockWriteGuard` намеренно ограничены скоупами без
  `.await` — иначе spawn-нутая future не была бы `Send`. См. `execute_probe` в
  `engine.rs`.
- UI получает прогресс/результаты событиями, а не опросом.

## Поток данных при сканировании

1. Vue собирает `ScanTarget` + `ScanParams`, вызывает `start_scan`.
2. `ScannerHandle::start` строит стратегию, планирует задачу движка, ставит статус.
3. Движок в цикле: `strategy.next_item()` → сборка ADU → транзакция транспорта →
   проверка unit id / функции / контрольной суммы → классификация ответа.
4. При ответе создаётся/обновляется `DeviceRecord` в общих результатах и
   отправляется событие `device_found` / `device_updated`.
5. Если есть план регистров, выполняются дополнительные `RegisterRead` и их
   данные складываются в `record.registers`.
6. События прогресса обновляют статус-бар; `get_scan_results` отдаёт полный
   снимок для таблицы результатов.

## Сохранение настроек

`Config` описан в `state.rs`; команды `get_config`/`save_config`/`load_config`
читают/пишут `config-v2.json` в `app_config_dir()` Tauri (например,
`%APPDATA%\com.modbus-scanner.app\config-v2.json` в Windows).

## Планируемые модули (v0.2+, см. [PLAN.ru.md](PLAN.ru.md))

| Модуль | Назначение |
|--------|------------|
| `scanner::strategies::BaudSweepStrategy` | свип baud/framing перед сканом адресов |
| `scanner::strategies::RegisterScanStrategy` | адаптивный поиск регистров Quick/Balanced/Deep |
| `scanner::classifier` | классификация IP: валидный Modbus / открытый порт / таймаут / недоступен |
| `modbus::device_id` | идентификация устройства FC43/MEI14 |
| `modbus::decode` | чистые функции декодирования (int/float, byte order, scaling) |
| `state::TagMap` | именованные регистры и масштабирование в проектах |
| `stores/monitor.ts`, события `monitor:update` | живой опрос и тренды |

## Граница с фреймворком (v1.0)

`scanner/` больше не импортирует Tauri. Ядро испускает события через `EventSink`,
а сырые обмены и выборки — через трейты `TrafficSink` / `SampleSink`. Единственный
модуль, знающий про Tauri, — `bridge.rs`: он строит `EventSink` из `AppHandle`.
Будущий CLI сможет передать `EventSink::null()` или свой emitter без изменений движка.

## Границы сборки

- `src-tauri/tauri.conf.json` — frontend dist `../dist`, окно, бандл, иконки.
- `src-tauri/capabilities/default.json` — права Tauri 2 (core:default, shell:allow-open).
- `src-tauri/build.rs` — `tauri_build::build()`.

## Границы актуальности архитектуры (21.11.2026)

Фактический тракт: Modbus-протокол и транспорт на Rust → движок сканирования с проверками безопасности → команды Tauri или опциональный API на loopback → интерфейс Vue/Pinia. При изменениях важно сохранить инварианты тайм-аутов, отмены и read-only обнаружения. Раздел `Планируемые модули (v0.2+)` ниже — **исторический проектный план**, а не актуальный перечень реализованных функций. Источник статуса — [ROADMAP](ROADMAP.md), код и тесты.

Отдельный пункт анализа безопасности: в `src-tauri/tauri.conf.json` сейчас `app.security.csp: null`. Подбор ограничивающей CSP требует анализа Tauri capabilities и ресурсов WebView с последующими тестами. Само по себе наблюдение не доказывает наличие эксплуатируемой уязвимости. См. [SECURITY](SECURITY.md), [аудит](AUDIT_2026-11-21.md).

PR #13: AddressSpace → RegisterKey → RegisterSpan → TagDefinition.id; идентификация не смешивает FC03/FC04. Декодирование инспектора использует только полный диапазон, ноль является корректным значением. Проекты v1 не мигрируют автоматически; отдельная конфигурация v2. [Модель данных](DATA_MODEL.md).

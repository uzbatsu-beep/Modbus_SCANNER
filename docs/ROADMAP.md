# Roadmap / Дорожная карта

> Обоснование приоритетов: [RESEARCH.md](RESEARCH.md) (исследование рынка,
> сообщества и научных работ). Детальные этапы и критерии готовности:
> [PLAN.md](PLAN.md). Приоритеты ниже синхронизированы с этими документами.

## Current status — MVP (v0.1.0)

**Done:**
- [x] Workspace: Tauri 2 (Rust) + Vue 3 + TypeScript + Vite + Pinia
- [x] Modbus protocol layer: PDU/ADU, RTU (CRC-16), ASCII (LRC), exception codes
- [x] Transports: serial (tokio-serial), TCP (MBAP), UDP (datagram ADU)
- [x] Scanner engine: strategies, control (start/stop/pause/resume), events, results
- [x] UI: four tabs (Scan, Results, Manual, Settings), live device table, register view
- [x] Manual console: read/write/raw frame
- [x] Port enumeration and port test
- [x] Export CSV / JSON / XML, config persistence
- [x] `cargo check`, `npm run typecheck`, protocol unit tests (see DEVELOPMENT for Windows-gnu caveat)

**Known gaps:**
- Scan params `request_gap_ms`, `ip_concurrency`, per-request `timeout_ms` and `retries`
  are exposed in the UI but have fixed effective values in the engine
- IP targets are scanned sequentially; no bounded-concurrency worker pool yet
- Register plan only works for serial targets in `single_device`; TCP/UDP deep scan
  needs a proper target builder
- Export from the device detail pane exports the whole session, not the selected device
- `ModbusError` variants from a raw TCP MBAP frame are not verified against the
  transaction id yet
- `probe_completed` event is reserved but never emitted
- Light theme and full localization are not implemented

## Next — v0.2 (core scanning)

- [ ] Wire `ScanParams` through the engine: per-request timeout, retries, inter-request gap
- [ ] Bounded-concurrency IP worker pool (`ip_concurrency`) with a transport per target
- [ ] Baud-rate sweep strategy: try all standard rates/parities on a bus
- [ ] MBAP transaction-id validation; "RTU over TCP" / MBAP-over-UDP selector
- [ ] Single-device deep scan for TCP/UDP endpoints
- [ ] Export per-device results from the detail pane
- [ ] Emit `probe_completed` events and per-probe log entries

## Later — v0.3+ (instrumentation)

- [ ] Serial-only diagnostics: FC08 (Diagnostics), FC07/0B/0C, device ID FC2B/0x2B
- [ ] Register map sweep: contiguous-range detection with configurable step
- [ ] Value decoding presets (float32, uint32, int16, scaled values, bitfields)
- [ ] Scan profiles / templates: saved targets + register plans
- [ ] Session compare (diff two scans)
- [ ] CSV/XLSX export with decoded columns
- [ ] Full i18n (RU/EN) and light theme
- [ ] Generate `src/types/ipc.ts` from Rust types (`ts-rs` or custom schema)
- [ ] Unit tests for strategies/engine with a mock transport
- [ ] Linux/macOS CI matrix

## Out of scope (for now)

- Mobile targets (Tauri Android/iOS) — desktop-only UI
- Modbus server/simulator mode
- Writing during discovery
- Integration with PLC programming suites

---

## Текущий статус — MVP (v0.1.0)

**Готово:** рабочее пространство Tauri 2 + Vue 3; протокольный слой (RTU/ASCII,
CRC/LRC, исключения); транспорты serial/TCP/UDP; движок сканера со стратегиями,
управлением и событиями; четыре вкладки UI; ручная консоль; перечисление портов;
экспорт и сохранение настроек; проверки сборки и тесты протокола (см. DEVELOPMENT
про особенности Windows-gnu).

**Известные пробелы:** параметры `request_gap_ms`, `ip_concurrency`, таймаут и
ретраи пока не влияют на движок; IP-цели сканируются последовательно; план
регистров работает только для последовательных целей; экспорт из панели устройства
экспортирует всю сессию; `probe_completed` зарезервировано, но не испускается;
светлая тема и полная локализация не реализованы.

## Ближайшие шаги (v0.2)

Подключить параметры сканирования к движку; пул воркеров с ограничением
параллелизма для IP; стратегия перебора скоростей/чётности; проверка transaction id
MBAP и выбор формата ADU для UDP/TCP; глубокое сканирование TCP/UDP; экспорт по
устройству; события `probe_completed`.

## Позже (v0.3+)

Диагностические функции (FC08/07/0B/0C, идентификация 2B), автоматический поиск
непрерывных диапазонов регистров, пресеты декодирования (float32, uint32, bitfields),
профили сканирования, сравнение сессий, полная локализация и светлая тема,
генерация TS-типов из Rust, тесты движка с mock-транспортом, CI для Linux/macOS.

## Вне текущего объёма

Мобильные платформы, режим Modbus-сервера/симулятора, запись в процессе
обнаружения, интеграция с пакетами программирования ПЛК.

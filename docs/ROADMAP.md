# Roadmap / Дорожная карта

> Обоснование приоритетов: [RESEARCH.md](RESEARCH.md) /
> [RESEARCH.ru.md](RESEARCH.ru.md) (исследование рынка, сообщества и научных работ).
> Детальные этапы и критерии готовности: [PLAN.md](PLAN.md) /
> [PLAN.ru.md](PLAN.ru.md). Приоритеты ниже синхронизированы с этими документами.

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

**In progress (Unreleased):**
- [x] P2-02: `timeout_ms`, `retries`, `request_gap_ms`, `ip_concurrency` are now
  effective (single `EngineParams`, clamped; serial sequential, TCP/UDP in a
  bounded pool). Also fixes register-plan reads on a serial bus reusing the port.

**Known gaps:**
- `single_device` register plan is still serial-only; for TCP/UDP use the new
  `register_scan` target
- [x] Export from the device detail pane exports only the selected device (`export_device`)
- `ModbusError` variants from a raw TCP MBAP frame are not verified against the
  transaction id yet
- `probe_completed` event is reserved but never emitted
- Light theme and full localization are not implemented

## Next — v0.2 (core scanning)

- [x] Wire `ScanParams` through the engine: per-request timeout, retries, inter-request gap
- [x] Bounded-concurrency IP worker pool (`ip_concurrency`) with a transport per target
- [x] Baud-rate sweep strategy: two-phase `serial_auto` (few ids across all settings, then full range on the matched one)
- [x] Adaptive register map (`register_scan`): Quick/Balanced/Deep, exception-aware splitting, timeout guard
- [x] IP target classification: valid Modbus / port open / refused / timeout / unreachable
- [x] FC43/MEI14 basic device identification (VendorName, ProductCode, Revision)
- [x] MBAP transaction-id validation; "RTU over TCP" / MBAP-over-UDP selector
- [ ] Single-device deep scan for TCP/UDP endpoints
- [ ] Export per-device results from the detail pane
- [x] Emit `probe_completed` events and per-probe log entries

## Later — v0.3+ (instrumentation)

- [ ] Serial-only diagnostics: FC08 (Diagnostics), FC07/0B/0C, device ID FC2B/0x2B
- [ ] Register map sweep: contiguous-range detection with configurable step
- [ ] Value decoding presets (float32, uint32, int16, scaled values, bitfields)
- [x] Scan profiles / templates: saved targets + register plans (projects, P3-04)
- [x] Session compare (diff two scans, P3-08)
- [ ] CSV/XLSX export with decoded columns
- [x] Full i18n (RU/EN) and light theme (P3-09)
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

**Выполнено в Unreleased:** P2-01 — двухфазный авто-свип `serial_auto`: сначала
несколько unit id на всех сочетаниях baud/чётности, после первого корректного
ответа — полный диапазон на найденных настройках. Скорость/чётность меняются с
переоткрытием общего последовательного порта.

P2-03 — адаптивная карта регистров `register_scan` (Quick/Balanced/Deep, сплит
неудачных блоков, исключения 01/03 → стоп, 3 таймаута → стоп).

P2-04 — классификация IP-целей: валидный Modbus / порт открыт, не Modbus /
отказ / таймаут / недоступен; отображается отдельной таблицей в результатах.

P2-05 — базовая идентификация FC43/MEI14: VendorName / ProductCode / Revision,
отображается в карточке устройства; управляется флагом `identify_devices`.

P2-08 — экспорт выбранного устройства (`export_device`): CSV/JSON/XML с зондами,
картой регистров и идентификацией FC43; общий экспорт сессии остался в
`export_results`.

P2-02 — параметры `timeout_ms`, `retries`, `request_gap_ms`, `ip_concurrency`
теперь влияют на движок (единый `EngineParams`, ограничения значений; serial —
последовательно, TCP/UDP — в ограниченном пуле). Заодно исправлено чтение плана
регистров на serial-шине с переиспользованием порта.

**Известные пробелы:** план регистров в `single_device` работает только для
последовательных целей (для TCP/UDP используйте `register_scan`);
светлая тема и полная локализация не реализованы.

## v0.2 — завершён

Все пункты выполнены: свип настроек, адаптивный скан регистров, классификация
IP-целей, FC43-идентификация, экспорт по устройству, валидация transaction id,
выбор формата ADU для TCP/UDP и события `probe_completed` с журналом зондов.

## Позже (v0.3+)

Диагностические функции (FC08/07/0B/0C), автоматический поиск
непрерывных диапазонов регистров, пресеты декодирования (float32, uint32, bitfields),
проекты со снимками сканов (P3-04), сравнение сессий, полная локализация и светлая тема,
генерация TS-типов из Rust, тесты движка с mock-транспортом, CI для Linux/macOS.

## Вне текущего объёма

Мобильные платформы, режим Modbus-сервера/симулятора, запись в процессе
обнаружения, интеграция с пакетами программирования ПЛК.

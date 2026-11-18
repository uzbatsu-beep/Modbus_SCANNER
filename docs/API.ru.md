# Справочник IPC API

[English version](API.md)

Фронтенд общается с Rust через Tauri 2: команды — `invoke`, события — `listen`.
Все ошибки команд приходят как читаемые строки `String`. Типизированные обёртки —
`src/api/mod.ts`, типы полезных данных — `src/types/ipc.ts` (зеркало
`src-tauri/src/scanner/types.rs`).

## Команды

### Система

| Команда | Аргументы | Возврат |
|---------|-----------|---------|
| `get_app_info` | — | `{name, version, rustc, debug}` |
| `get_log_level` | — | `string` (значение `RUST_LOG`) |

### Последовательные порты

**`list_serial_ports`** → `SerialPortInfo[]`

```ts
{ port_name: string;
  port_type: "usb" | "pci" | "bluetooth" | "unknown";
  vid?: number; pid?: number;
  manufacturer?: string; product?: string }
```

**`test_serial_port`** — аргументы: `{ params: TestSerialParams }`

```ts
{ port_name: string; baud_rate: number;
  data_bits: 5|6|7|8; parity: "none"|"even"|"odd"; stop_bits: 1|2;
  mode: "rtu"|"ascii"; test_modbus: boolean }
```

Возвращает `{ opened, error?, modbus_responded, rtt_us?, response_hex? }`.
Тест отправляет FC03 (unit 1, адрес 0, количество 1) и ждёт ответ 800 мс.

### Управление сканером

| Команда | Аргументы | Возврат | Действие |
|---------|-----------|---------|----------|
| `start_scan` | `{ target: ScanTarget, params: ScanParams }` | `void` | Запускает задачу движка, возвращается сразу |
| `stop_scan` | — | `void` | Останавливает после текущего запроса |
| `pause_scan` | — | `void` | Пауза между элементами работы |
| `resume_scan` | — | `void` | Продолжить |
| `get_scan_status` | — | `ScanStatus` | `"idle" \| "preparing" \| "running" \| "paused" \| "stopped" \| "finished" \| "failed"` |
| `get_scan_progress` | — | `ScanProgress` | Последний снимок прогресса |
| `get_scan_results` | — | `ScanResults` | Полный снимок результатов |
| `clear_results` | — | `void` | Очищает записи и статистику |
| `export_results` | `{ path: string, format: "csv"\|"json"\|"xml" }` | `number` | Пишет файл со всей сессией, возвращает число устройств |
| `export_device` | `{ key: string, path: string, format: "csv"\|"json"\|"xml" }` | `boolean` | P2-08: экспортирует только выбранное устройство (зонды, регистры, FC43-идентификацию); ошибка, если ключ неизвестен |
| `save_project` | `{ path: string, name: string }` | `string` (путь) | P3-04: пишет устройства + теги + историю сканов в один JSON; добавляет снимок (макс. 20) |
| `load_project` | `{ path: string }` | `Project` | P3-04: восстанавливает результаты и теги, синхронизирует `config.json` |
| `start_monitor` | `{ watches: MonitorWatch[], interval_ms: number }` | `void` | P3-05: опрос регистров каждые 250–10000 мс |
| `stop_monitor` | — | `void` | Остановить опрос |
| `get_monitor_state` | — | `MonitorValueState[]` | Текущие min/max/изменения и история выборок |
| `get_traffic_log` | — | `TrafficRecord[]` | P3-06: последние 2000 сырых обменов |
| `clear_traffic_log` | — | `void` | Очистить журнал трафика |
| `export_traffic_log` | `{ path: string, format: "csv"\|"json" }` | `number` | Экспорт журнала (число записей) |

Варианты `ScanTarget`:

```ts
// последовательная шина с известными настройками
{ type: "unit_id_range", from: number, to: number,
  port: { port_name, baud_rate, data_bits, parity, stop_bits },
  mode: "rtu" | "ascii" }

// последовательная шина с неизвестными настройками (P2-01): двухфазный свип
{ type: "serial_auto",
  sweep: {
    port_name: string, mode: "rtu" | "ascii",
    baud_rates: number[],                 // в порядке приоритета
    framings: Array<{ data_bits, parity, stop_bits }>, // в порядке приоритета
    discovery_unit_ids: number[],         // несколько id для фазы 1
    unit_from: number, unit_to: number    // полный диапазон для фазы 2
  } }

// адаптивная карта регистров/coil на одном устройстве (P2-03)
{ type: "register_scan",
  scan: {
    target: ManualTarget,
    function: 1|2|3|4,
    start: number, end: number,       // включительно
    mode: "quick" | "balanced" | "deep",
    batch: number                      // 0 = значение по умолчанию режима
  } }

// диапазон IP (from может быть CIDR; to = null для одного адреса/подсети)
{ type: "ip_range", from: string, to: string | null,
  port: number, kind: "tcp" | "udp" }

// одно устройство + план регистров
{ type: "single_device", target: DeviceTarget,
  register_plan: Array<{ function, start, quantity }> }
```

`ScanParams`:

```ts
{ timeout_ms: number; request_gap_ms: number; retries: number;
  probe_functions: number[]; stop_on_first: boolean;
  register_plan: Array<{ function, start, quantity }>;
  ip_concurrency: number; identify_devices?: boolean;
  tcp_framing?: "mbap"|"rtu"; udp_mbap?: boolean }
```

### Ручные команды Modbus

**`read_registers`** — `{ req: ReadRequest }`

```ts
{ target: ManualTarget, function: 1|2|3|4,
  start_address: number, quantity: number,
  timing?: { timeout_ms: number, retries?: number } }
```

**`write_registers`** — `{ req: WriteRequest }`, та же форма с
`function: 5|6|16` и `values: number[]`.

**`inspect_registers`** — `{ values: number[] }` → `DecodeResult`: все
интерпретации (INT16/UINT16/INT32/UINT32/FLOAT32/ASCII × ABCD/BADC/CDAB/DCBA)
и помеченные гипотезы float. **`decode_register`** —
`{ values: number[], spec: DecodeSpec }` → `DecodedValue | null` (для именованных
тегов; `DecodeSpec` = `{ data_type, byte_order, multiplier?, divider?, offset? }`).

**`send_raw_frame`** — `{ req: RawFrameRequest }`

```ts
{ target: ManualTarget, adu_hex: string,
  is_full_adu: boolean, timing?: ManualTiming }
```

`is_full_adu = false`: `adu_hex` — это PDU, ADU собирается автоматически
(RTU: +unit+CRC; ASCII: с LRC; TCP: с MBAP). `true` — байты уходят как есть.

Все три возвращают `ManualResponse`:

```ts
{ ok: boolean; error?: string; exception_code?: number;
  values: number[]; response_hex?: string; rtt_us?: number }
```

`ManualTarget`:
`{ kind, endpoint, port?, unit_id, serial?, serial_mode? }` — для RTU/ASCII
`endpoint` — имя порта, а `serial` должен содержать параметры порта.

### Конфигурация

| Команда | Аргументы | Возврат |
|---------|-----------|---------|
| `get_config` | — | `Config` из памяти |
| `save_config` | `{ config: Config }` | `void` (также пишет `config.json`) |
| `load_config` | — | `Config` (с диска или значения по умолчанию) |

Поля `Config`: `language`, `theme`, `scan_params`, `last_serial`,
`last_tcp_host`, `last_tcp_port`, `last_udp_host`, `last_udp_port`,
`export_dir`, `export_format`, `tags` (именованные теги регистров, P3-03).

## События

Подписка через `listen` (обёртки `onScanProgress` / `onScanEvent` в `src/api/mod.ts`).

### `scan:progress` — `ScanProgress`

```ts
{ status: ScanStatus; total: number; done: number;
  found: number; current: string; errors: number;
  elapsed_ms: number }
```

Отправляется после каждого элемента работы. `total/done` — счётчики запросов.

### `scan:event` — `ScanEvent`

Объединение с дискриминатором `kind`:

```ts
{ kind: "status_changed", status: ScanStatus, message: string }
{ kind: "device_found", record: DeviceRecord }
{ kind: "device_updated", record: DeviceRecord }
{ kind: "ip_target_classified", status: IpTargetStatus }
{ kind: "probe_completed", device_key, function, result: ProbeResult }
{ kind: "finished", total, found, errors, elapsed_ms }
```

`probe_completed` отправляется один раз на каждый завершённый зонд (включая
таймауты и транспортные ошибки); фронтенд пишет одну строку в журнал событий.

`DeviceRecord`:

```ts
{ key: string;                       // "rtu:COM3:1", "tcp:1.2.3.4:502:1"
  target: DeviceTarget;
  probes: Record<number, ProbeResult>;
  registers: Record<string, RegisterValue>;  // ключ "fc:address", напр. "03:0000"
  device_identification?: DeviceIdentification | null;
  first_seen_ms: number; last_seen_ms: number }

// FC43/MEI14, P2-05
interface DeviceIdentification {
  read_code: number; conformity_level: number;
  more_follows: boolean; next_object_id: number;
  objects: Array<{ id: number; value: string }>
}
```

Варианты `ProbeResult` (дискриминатор `status`):

```ts
{ status: "ok", function, values: number[], raw_hex: string, rtt_us: number }
{ status: "exception", function, code: number, message: string, rtt_us: number }
{ status: "no_response", function }
{ status: "transport_error", function, message: string }
```

`RegisterValue`: `{ function, address, raw, as_hex, rtt_us }`.

`Project` (P3-04): `{ name, version, created_ms, updated_ms,
results: ScanResults, tags: TagDefinition[], history: ScanSnapshot[] }`, где
`ScanSnapshot = { timestamp_ms, label, results }`.

### `monitor:update` — `MonitorUpdate` (P3-05)

`{ values: MonitorValueState[], cycle_ms: number }` — отправляется один раз за
цикл опроса. `MonitorValueState` содержит `key`, `name`, `unit`, последние
`values`, `min`/`max`, флаг `changed` и до 5000 `samples` (окно один час).

### `traffic:record` — `TrafficRecord` (P3-06)

`{ id, timestamp_ms, kind, endpoint, unit_id, function?, tx_hex, rx_hex?, rtt_us,
status }` — один сырой обмен на зонд/чтение, включая неудачные. UI хранит
последние 2000 записей, умеет фильтровать по FC/статусу и экспортировать CSV/JSON.

### Классификация IP-целей (P2-04)

`ScanResults.ip_targets: IpTargetStatus[]` — по одной записи на каждый `ip:port`:

```ts
{ endpoint: string; port: number;
  class: "valid_modbus" | "port_open_no_modbus" | "connection_refused"
       | "timeout" | "unreachable" | "unknown";
  unit_ids_probed: number[];
  responses_ok: number; responses_exception: number;
  timeouts: number; transport_errors: number;
  last_seen_ms: number }
```

Класс не понижается: корректный ответ важнее открытого порта, тот — отказа/
таймаута/недоступности. Открытый TCP-сокет без корректного Modbus-ответа
показывается как `port_open_no_modbus`, а не как устройство.

## Пример использования из фронтенда

```ts
import { api, onScanEvent, onScanProgress } from "@/api/mod";

await onScanProgress((p) => console.log(p.done, "/", p.total));
await onScanEvent((e) => {
  if (e.kind === "device_found") console.log("найдено:", e.record.key);
});

await api.scanner.start(
  { type: "unit_id_range", from: 1, to: 247, mode: "rtu",
    port: { port_name: "COM3", baud_rate: 9600, data_bits: "eight",
            parity: "none", stop_bits: "one" } },
  { timeout_ms: 800, request_gap_ms: 20, retries: 1,
    probe_functions: [3, 4, 1, 2], stop_on_first: false,
    register_plan: [], ip_concurrency: 32 }
);
```

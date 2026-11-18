# Scanning

[Русская версия](SCANNING.ru.md)

## Scan targets

### 1. Serial bus (`unit_id_range`)

One serial port, unit ids `from…to` (0–247). For every id, the engine sends each
configured probe FC (default: 03, 04, 01, 02). One `SerialTransport` is opened
lazily and reused for the entire scan — this preserves the bus arbitration and
the t3.5 gap between frames.

Use this when the serial settings are known.

### 1b. Serial bus with unknown settings (`serial_auto`, P2-01)

Enable “Авто-поиск настроек” in the serial form. The engine performs a **two-phase
sweep**:

1. **Phase 1 — settings detection.** For each framing (8N1 → 8E1 → 8O1) and
   each baud rate in priority order (9600 → 2400 → 19200 → 4800 → 38400 →
   57600 → 115200 → 1200), it probes only a few likely unit ids:
   1, 2, 3, 4, 5, 10, 20, 100, 247. A frame with a valid CRC/LRC — including a
   Modbus exception — counts as an answer.
2. **Phase 2 — full scan.** The matching baud/framing is locked, the shared
   serial port is reopened with those settings, and the complete unit-id range
   is scanned. Other settings are never tried once a device answered.

The UI lets you limit which baud rates and parities participate. With all
defaults the worst case is 8 × 3 × 9 = 216 short probes; a response timeout of
200–300 ms is recommended for sweep scans.

Unit id 0 is broadcast: devices do not answer, so do not include it unless you
intentionally test broadcast behaviour.

### 2. IP range (`ip_range`)

IPv4 range (`from`–`to`) or CIDR (`from = "192.168.1.0/24"`, empty `to`).
The range is limited to 65536 addresses. Ports: 502 by default, but field
devices often use 503/1502/802.

For each IP the engine probes unit ids **1 and 255** (the common direct-TCP and
gateway passthrough ids). Modbus TCP gateways route to serial slaves by unit id;
if you need ids 2–247, use a single-device scan with an explicit unit id.

TCP = MBAP-framed request; UDP = RTU-style ADU in one datagram (no response
means no device). `TcpTransport` uses a fresh connection per probe, which keeps
the engine simple and tolerates devices that close sockets aggressively.

### 2b. IP target classes (P2-04)

Every `ip:port` gets a classification in `ScanResults.ip_targets` (also visible
in the Results tab):

| Class | Meaning |
|-------|---------|
| `valid_modbus` | At least one valid Modbus frame (data or exception) |
| `port_open_no_modbus` | TCP handshake succeeded, but nothing answered in Modbus |
| `connection_refused` | TCP connection actively refused |
| `timeout` | No answer within the configured timeout |
| `unreachable` | Host/network unreachable |

Classes are ranked and never downgrade while unit ids are probed behind one IP.
A refused/unreachable/timeout target does **not** create a fake device record;
it only appears in the classification table. For UDP there is no handshake, so
the only positive class is `valid_modbus` and silence is reported as `timeout`.

### 3. Single device (`single_device`)

One endpoint + explicit unit id, with a `register_plan` of ranges to read.
Each found device gets its `registers` map filled with the results.

### 3b. Adaptive register map (`register_scan`, P2-03)

A dedicated mode for “the device answers, but I have no register manual”.
It reads a whole address range with block requests and adapts to failures:

| Mode | Behaviour |
|------|-----------|
| `Quick` | Blocks of `batch` (default 100); a failed block is split into fifths once, then edges are refined to single registers. Fastest on contiguous maps. |
| `Balanced` (default) | 100 → 20 → 5 → single registers. Good compromise for mixed maps. |
| `Deep` | Keeps splitting failed blocks until single registers. Finds isolated registers, but issues the most requests. |

Rules shared by all modes:

- exception **0x02** means “this address is absent” → split the block;
- exception **0x01** (unsupported function) or **0x03** → abort immediately;
- **timeouts are never split** — retrying sub-blocks would cost a full timeout
  each; three consecutive timeouts abort the scan;
- addresses that answered are remembered, so edge refinement never re-reads
  a known readable register;
- results are written into the device record even if the device was not found
  by a previous discovery scan.

The UI field “Размер блока” overrides the mode default; values are clamped to
the function's spec limit (125 registers or 2000 coils).

## Scan parameters

| Parameter | Default | Meaning |
|-----------|---------|---------|
| `timeout_ms` | 800 | Response timeout per request (ms) |
| `request_gap_ms` | 20 | Pause between consecutive requests (ms) |
| `retries` | 1 | Retries before classifying as "no response" |
| `probe_functions` | `[3, 4, 1, 2]` | FCs used to probe device presence |
| `stop_on_first` | false | Stop after the first found device |
| `register_plan` | `[]` | Ranges to read on each found device |
| `ip_concurrency` | 32 | Planned max in-flight IP probes |

**Implementation note:** all four parameters are effective: the engine derives
one `EngineParams` set at scan start (timeout clamped to 50–10000 ms, retries
≤ 5, gap ≤ 1000 ms, IP concurrency 1–256). Serial probes stay strictly
sequential; TCP/UDP probes run in a bounded worker pool. The request gap is
applied after every work item; Pause stops scheduling new work.

## Timing recommendations

| Scenario | Timeout | Gap | Notes |
|----------|---------|-----|-------|
| RTU @ 9600, 8N1 | 500–1000 ms | 20–50 ms | slow devices with CRC recomputation |
| RTU @ 115200 | 200–500 ms | 5–10 ms | t3.5 ≈ 1.75 ms per spec |
| ASCII @ 9600 | 1000–2000 ms | 50 ms | text framing + CRLF |
| TCP on LAN | 200–500 ms | 0–10 ms | no serial arbitration |
| UDP | 100–300 ms | 0 ms | connectionless: no reply is final |
| RTU bus with 30+ slaves | 300–600 ms | 20 ms | full range takes minutes — plan time |

Rule of thumb: timeout > (response time of the slowest device) × 1.5. Too short
a timeout creates false negatives; too long makes a 247-id scan painfully slow.

## How detection works

1. Probe PDU: read 1 element from address 0 using the configured FC.
2. Any valid Modbus response (data **or** exception) counts as "device found".
3. A CRC/LRC error, unit-id mismatch, unexpected FC, or timeout does not.
4. Results are accumulated per device key:
   `rtu:COM3:1`, `tcp:192.168.1.5:502:1`, `udp:…`.
5. Found devices are pushed to the UI immediately via `scan:event`;
   the results table can be refreshed live.

## Interpreting results

- `OK rtt` — device answered with data; hex values are shown in the detail pane.
- `EXC 01/02/…` — device answered with an exception. Common case: address 0 is
  not implemented → **Illegal data address (0x02)**. The device is still alive.
- `no_response` — timeout after retries. Could be a wrong unit id, wrong baud,
  wrong parity, or no device.
- `transport_error` — port could not be opened, connection refused, malformed
  frame. Check cabling/port permissions/firewall.

## Field practice (from the research)

See [RESEARCH.md](RESEARCH.md) for the full source list. The following practices
are consistently recommended in field guides and will be implemented in v0.2:

- **Unknown serial settings:** sweep baud × framing before sweeping unit IDs.
  Priority order: 9600 8N1 → 2400 8N1 → 19200 8N1 → even-parity variants (8E1).
  Stop at the first combination that produces a valid CRC.
- **Duplicate addresses:** two slaves on the same address answer simultaneously,
  producing CRC collisions. A CRC error must never be reported as a device.
  Recommendation: connect devices one at a time (point-to-point) to set addresses.
- **One master only:** scanning a bus while a PLC is already polling produces
  unusable results — disconnect the master or scan in a quiet window.
- **Exception 02 (Illegal Data Address)** is a positive signal: the device is
  alive, only the probed address is absent.
- **Open port 502 is not a device** — the response must be valid Modbus.

## Safety

- The scanner only issues **read** function codes (01–04) during discovery.
- Write functions (05/06/0F/10) are restricted to the Manual tab and require an
  explicit user action. See [SECURITY.md](SECURITY.md).
- Do not scan unit id 0 expecting a reply; broadcast writes in the Manual tab can
  affect **all** devices on a serial bus.
- On gateways, probing unit ids 2–247 may reach real serial slaves. Use care.

## Progress and events

`scan:progress` carries `{status, total, done, found, current, errors, elapsed_ms}`.
`scan:event` carries status changes, device found/updated, and a final summary.
See [API.md](API.md) for the full schema.

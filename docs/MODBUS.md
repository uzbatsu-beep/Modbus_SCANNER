# Modbus protocol reference

[Русская версия](MODBUS.ru.md)

This document describes exactly what the scanner implements. For the abstract
software architecture see [ARCHITECTURE.md](ARCHITECTURE.md).

## 1. Data model and addressing

Four classic data tables, each with 65536 elements:

| Table | Access | Function codes |
|-------|--------|----------------|
| Coils (0XXXX) | read/write | 01, 05, 15 |
| Discrete inputs (1XXXX) | read-only | 02 |
| Holding registers (4XXXX) | read/write | 03, 06, 10, 16, 17 (hex: 03, 06, 0x10, 0x16, 0x17) |
| Input registers (3XXXX) | read-only | 04 |

**Important:** the 1-based "Modbus notation" (40001 etc.) is *documentation
convention only*. The PDU always carries a **0-based** address (`u16`, 0…65535).
The UI edits 0-based addresses; convert by subtracting 1 if you use a device
manual written in 4XXXX notation.

Limits per request (spec):

| Function | Max quantity |
|----------|--------------|
| 01 / 02 (bits) | 2000 |
| 03 / 04 (registers) | 125 |
| 0F (write coils) | 1968 |
| 10 (write registers) | 123 |

## 2. PDU

```
PDU = [function code (1 byte)] [data (0…252 bytes)]
```

- Normal response: function code echoed, then data.
- Exception response: function code **| 0x80**, then one exception code byte.

## 3. ADU variants

### RTU (serial, binary)

```
[slave id (1)][PDU (1…253)][CRC-16 lo (1)][CRC-16 hi (1)]
```

Max ADU size = 256 bytes. CRC-16/Modbus: polynomial `0xA001` (reflected
`0x8005`), initial value `0xFFFF`, no final XOR; transmitted **low byte first**.
Slave id `0` = broadcast (no response; therefore a scan of id 0 always yields
"no response"). Valid slave ids for a scan: 1–247; 248–255 are reserved.

Framing relies on silence: a gap of **≥3.5 character times (t3.5)** delimits
frames. At ≥19200 baud the spec fixes t3.5 = 1.75 ms. `SerialTransport`
additionally uses a 5 ms silence detector before considering an RTU frame
complete, and an inter-frame gap before transmitting.

### ASCII (serial, hex text)

```
':' [slave id hex (2)][PDU hex (…)] [LRC hex (2)] CR LF
```

Every byte becomes two uppercase hex characters. LRC = two's complement of the
sum of all payload bytes (slave id + PDU), computed as `wrapping_neg()` of the
`u8` sum. The scanner tolerates lowercase hex when decoding. Frame end is CRLF;
a 1-second silence rule applies in the field.

### TCP (MBAP)

```
[transaction id (2)][protocol id (2)=0][length (2)][unit id (1)][PDU]
```

All fields big-endian. `length` = number of bytes *after* it (unit id + PDU),
so a read-single-register request has `length = 1 + 5 = 6`.
The scanner generates an increasing transaction id per request and **validates
that the response echoes it** (a mismatch is a `TransactionIdMismatch` error,
never a device record). Protocol id must be 0; TCP port 502 is standard
(installations often use 503, 1502, 802). Besides standard MBAP, the scanner can
send a raw RTU ADU over TCP (`tcp_framing = "rtu"`, common with serial gateways);
in that mode there is no MBAP header to validate. Unit id is used for routing through gateways; direct TCP
devices commonly answer only on unit 1, 0 or 255.

### UDP

Not part of the original Modbus spec, but widely used. Two payload forms exist
in the field:

1. **MBAP + PDU** (most common, identical to TCP ADU);
2. **RTU-style ADU** (slave id + PDU + CRC).

`UdpTransport` treats one datagram as one ADU. No response within the timeout
is the normal negative result. By default the scanner builds RTU-style ADUs for UDP;
with `udp_mbap = true` it uses the MBAP form instead and applies the same
transaction-id validation as TCP.

## 4. Function codes used by the scanner

| Code | Name | Direction | Scan role |
|------|------|-----------|-----------|
| 0x01 | Read Coils | read | probe + register map |
| 0x02 | Read Discrete Inputs | read | probe + register map |
| 0x03 | Read Holding Registers | read | default probe + register map |
| 0x04 | Read Input Registers | read | probe + register map |
| 0x05 | Write Single Coil | write | manual console |
| 0x06 | Write Single Register | write | manual console |
| 0x0F | Write Multiple Coils | write | manual console |
| 0x10 | Write Multiple Registers | write | manual console |
| 0x16 | Mask Write Register | write | reserved in `FunctionCode` only — no builder/parser yet |
| 0x17 | Read/Write Multiple Registers | read/write | reserved in `FunctionCode` only — no builder/parser yet |
| 0x2B | Read Device Identification (MEI 0x0E) | read | basic identification of found devices (P2-05) |

The diagnostic codes (0x07, 0x08, 0x0B, 0x0C) and device identification 0x2B
are serial-only in the spec and are **not yet exposed** in the UI (see ROADMAP).

## 4a. Modbus Security / TLS (E-05)

The official Modbus Security specification wraps the MBAP ADU in TLS on port
**802**. `ScanParams.tls = { client_cert?, client_key? }` enables it; the same
MBAP framing and transaction-id validation apply inside the tunnel. Server
certificates are verified against the webpki root set; an optional PEM client
certificate/key enables mutual TLS. Self-signed devices need their CA trusted
by the system. Set `tcp_framing = "rtu"` only if the gateway expects a raw RTU
frame (not typical for Modbus Security).

## 4b. Device identification (FC43 / MEI 0x0E, P2-05)

Request PDU (read category 1 = basic identification):

```
[0x2B][0x0E][0x01][0x00]
      MEI  read object id (0 = first object)
```

Response data (after the function code):

```
[MEI=0x0E][read code][conformity level][more follows][next object id][count]
  then `count` objects:
  [object id (1)][length (1)][value (length bytes, text)]
```

Standard basic objects: `0` VendorName, `1` ProductCode, `2` MajorMinorRevision.
Category 2 (regular) adds VendorUrl, ProductName, ModelName, UserAppName.
Conformity level 0x01 = basic only, 0x02 = regular, 0x03 = extended.
The scanner asks for basic identification once per found device (best effort):
an exception or malformed response simply leaves `device_identification` empty.

## 5. Response decoding

**Bits (FC 01/02):** first payload byte = byte count, then packed bits, LSB
first:

```
[count=2][b7..b0 of 0..7][b7..b0 of 8..15]...
```

**Registers (FC 03/04/17):** first payload byte = byte count (even), then big
endian `u16` values.

The UI shows registers as hex, decimal and 16-bit binary.

### Strict response validation (PR #5)

All read paths (discovery, adaptive map, manual read, live monitor) now share
the same PDU payload validator:

- FC01/FC02: 1–2000 requested bits; response byte count **exactly**
  `ceil(quantity/8)`, with exactly that many payload bytes; final padding bits
  may be ignored.
- FC03/FC04: 1–125 requested registers; response byte count **exactly**
  `2 × quantity`, even, within 250 bytes, no missing or trailing bytes.
- PDU length at most 253 bytes; exception PDU **exactly 2 bytes** with the
  requested base FC and a standard exception code; a mismatched exception
  is **not** evidence that a Modbus device was found.
- TCP/MBAP verification also checks Transaction ID, Protocol ID 0, exact MBAP
  length and Unit ID, including for manual requests.
- FC43 object count must consume its declared data exactly.

Malformed responses return an error and **never** become empty successful
reads, create fake register values or inflate successful-response statistics.
Adaptive register exploration aborts on invalid PDU rather than issuing
further divide-and-retry requests against a misbehaving device.

These checks validate the Modbus application framing and payload semantics.
They do not guarantee that a correctly encoded value matches the true
engineering quantity, type, scaling or equipment state.

## 5b. Multi-register values and byte order (P3-01)

Two consecutive registers `w0 w1` (each transmitted big-endian) can form a
32-bit value in four layouts:

| Name | Assembly |
|------|----------|
| ABCD | `(w0 << 16) \\| w1` — standard big-endian |
| BADC | bytes swapped inside each word |
| CDAB | words swapped |
| DCBA | words and bytes swapped |

The inspector shows all interpretations and marks plausible float32 results as
**hypotheses** (e.g. “possible float32 (ABCD)”), never as facts. Named tags add
engineering scaling `(raw × multiplier ÷ divider) + offset` and a unit.

## 6. Exception codes

`ModbusError::from_exception` maps all standard codes:

| Code | Meaning |
|------|---------|
| 0x01 | Illegal function |
| 0x02 | Illegal data address |
| 0x03 | Illegal data value |
| 0x04 | Slave device failure |
| 0x05 | Acknowledge |
| 0x06 | Slave device busy |
| 0x07 | Negative acknowledge |
| 0x08 | Memory parity error |
| 0x0A | Gateway path unavailable |
| 0x0B | Gateway target device failed to respond |

An exception response still proves that the device is alive; the scanner marks
such a probe as "device found", with an `exception` tag in the UI.

## 7. Verification performed by the engine

For every response, in order:

1. checksum verification (CRC for RTU/UDP, LRC for ASCII);
2. unit id match against the request (TCP: byte 6 of MBAP);
3. function code match (or a proper `fc | 0x80` exception);
4. MBAP protocol id == 0 and plausible length (TCP);
5. payload structure and byte-count consistency.

Mismatches are reported as transport errors and do not create a device record.

### Safe manual writes and RTU-over-TCP (PR #6)

- FC05 and FC06 write acknowledgments must **exactly echo** the requested
  address and value; FC10 must echo the requested start address and quantity.
  A function code match alone is not sufficient to confirm a write.
- Empty or oversized Raw Frame payloads are blocked before TX; raw responses
  are not validated as write acknowledgments even when bytes are received.
- Write and arbitrary Raw Frame actions are never automatically replayed,
  including after a lost reply, timeout, or an I/O error; only reads can retry.
- A timeout is an **unknown write outcome**, not proof that the device did not
  execute the command. The operator must read back state before sending a new
  command. This is NOT an emergency stop or an exactly-once delivery guarantee.
- RTU-over-TCP waits for the first response byte up to the transaction timeout,
  uses length-aware receive framing for common FCs and FC43, and enforces the
  256-byte RTU ADU maximum. TLS handshake uses a bounded timeout.
- Serial RTU receive buffers are capped at 256 bytes and ASCII at 513 bytes;
  oversized or unterminated frames produce a protocol error without unbounded
  memory growth.

## 7b. OT scan authorization and quiet pacing (PR #7)

An empty `ip_allowlist` denies TCP/UDP scan targets; explicit IP/CIDR
permission is needed before socket creation. Exclusions override permissions.
All non-excluded IPs in a scan range must be authorized; single-device and
register-scan targets are checked with the same policy. Hostnames are
disallowed for scan destinations to avoid DNS-rebinding bypasses.

Quiet Mode uses separate 500 ms shared gates for **Modbus TX** and **TCP
connection attempts**; the effective IP concurrency is forced to 1 to
avoid simultaneous SYN bursts. Both gates are per scan. Transmission slots
include retries, register reads and FC43; refused TCP connections consume a
connection slot even when no PDU is sent. An already transmitted Modbus request cannot
be unsent; the limit is not a deterministic network scheduling guarantee
on arbitrary industrial gateways.

## 8. Reference frames (test vectors)

Read holding registers, unit 1, address 0, quantity 10:

```
PDU: 03 00 00 00 0A
RTU: 01 03 00 00 00 0A C5 CD
ASCII: :01030000000AF2\r\n
TCP MBAP: 00 01 00 00 00 06 01 03 00 00 00 0A
```

These vectors are covered by unit tests in `rtu.rs`, `ascii.rs` and `frame.rs`.

## Implementation and validation boundary (2026-11-21)

Protocol reference describes intended and implemented frame behavior, but a successful loopback unit test does not establish real serial turnaround timing, termination, grounding or electromagnetic compatibility. For RTU/ASCII test evidence use [simulator scope](TEST_SIMULATOR.md) and [physical bench matrix](FIELD_VALIDATION_v1.0.1.md). **Discovery sends only read functions; manual FC05/06/10 writes must not be folded into automatic discovery.** See [OT security](SECURITY.md), [current audit](AUDIT_2026-11-21.md).

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
The server echoes the transaction id (the scanner matches requests to responses
by it); protocol id must be 0; TCP port 502 is standard (installations often
use 503, 1502, 802). Unit id is used for routing through gateways; direct TCP
devices commonly answer only on unit 1, 0 or 255.

### UDP

Not part of the original Modbus spec, but widely used. Two payload forms exist
in the field:

1. **MBAP + PDU** (most common, identical to TCP ADU);
2. **RTU-style ADU** (slave id + PDU + CRC).

`UdpTransport` treats one datagram as one ADU. No response within the timeout
is the normal negative result. The scanner currently builds RTU-style ADUs for
UDP (see ROADMAP for MBAP-over-UDP selection).

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
| 0x16 | Mask Write Register | write | code supported in the codec |
| 0x17 | Read/Write Multiple Registers | read/write | code supported in the codec |

The diagnostic codes (0x07, 0x08, 0x0B, 0x0C) and device identification 0x2B
are serial-only in the spec and are **not yet exposed** in the UI (see ROADMAP).

## 5. Response decoding

**Bits (FC 01/02):** first payload byte = byte count, then packed bits, LSB
first:

```
[count=2][b7..b0 of 0..7][b7..b0 of 8..15]...
```

**Registers (FC 03/04/17):** first payload byte = byte count (even), then big
endian `u16` values.

The UI shows registers as hex, decimal and 16-bit binary.

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

## 8. Reference frames (test vectors)

Read holding registers, unit 1, address 0, quantity 10:

```
PDU: 03 00 00 00 0A
RTU: 01 03 00 00 00 0A C5 CD
ASCII: :01030000000AF2\r\n
TCP MBAP: 00 01 00 00 00 06 01 03 00 00 00 0A
```

These vectors are covered by unit tests in `rtu.rs`, `ascii.rs` and `frame.rs`.

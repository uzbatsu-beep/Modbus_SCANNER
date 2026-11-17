//! Modbus ASCII framing: ':' + hex(ADU) + LRC + CR LF.
//!
//! Frame layout (on the wire):
//! `':' [slave id (2 hex)] [function (2 hex)] [data (0..2*253 hex)] [LRC (2 hex)] CR LF`
//!
//! Notes for the scanner:
//! - 1 byte is transmitted as 2 ASCII hex chars (0-9, A-F, upper case per spec);
//! - LRC = 8-bit two's complement of the sum of all payload bytes (excl. ':' and CRLF);
//! - inter-frame gap >= 1 s of silence (t3.5 in ASCII mode = 1.5 char * 11 bits... in practice 1 s);
//! - no broadcast in the strict sense, but unit 0 is still used as broadcast.

use super::common::{ModbusError, UnitId};
use super::frame::{AsciiAdu, Pdu};

/// ASCII start-of-frame character.
pub const SOF: u8 = b':';
/// ASCII end-of-frame sequence.
pub const EOF: [u8; 2] = [b'\r', b'\n'];

/// Compute 8-bit LRC (two's complement of the sum of bytes).
pub fn l8(data: &[u8]) -> u8 {
    let sum: u8 = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    sum.wrapping_neg()
}

/// Encode a raw wire frame (with ':' ... CRLF) from unit id + raw PDU bytes.
pub fn encode(unit_id: UnitId, pdu: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(1 + pdu.len());
    payload.push(unit_id.0);
    payload.extend_from_slice(pdu);
    let lrc = l8(&payload);
    payload.push(lrc);

    let mut frame = Vec::with_capacity(3 + payload.len() * 2);
    frame.push(SOF);
    let payload_hex = uppercase_hex(&payload);
    frame.extend_from_slice(&payload_hex);
    frame.extend_from_slice(&EOF);
    frame
}

/// Parse and verify a full ASCII wire frame (with ':' and CRLF).
pub fn decode(frame: &[u8]) -> Result<AsciiAdu, ModbusError> {
    if frame.len() < 5 {
        return Err(ModbusError::TooShort(frame.len()));
    }
    if frame[0] != SOF {
        return Err(ModbusError::InvalidFrame("missing leading ':'".into()));
    }
    let end = frame.len() - 2;
    if frame[end..] != EOF {
        return Err(ModbusError::InvalidFrame("missing trailing CRLF".into()));
    }
    let body = &frame[1..end];
    if body.len() < 6 || body.len() % 2 != 0 {
        return Err(ModbusError::InvalidFrame(format!(
            "bad hex body length {}",
            body.len()
        )));
    }
    let payload = parse_hex(body)
        .map_err(|e| ModbusError::InvalidFrame(format!("invalid hex: {e}")))?;

    // payload: [unit id] [pdu...] [lrc]
    let (data, lrc_bytes) = payload.split_at(payload.len() - 1);
    let expected = lrc_bytes[0];
    let actual = l8(data);
    if expected != actual {
        return Err(ModbusError::LrcMismatch { expected, actual });
    }

    let unit_id = UnitId(data[0]);
    let pdu = Pdu::from_bytes(&data[1..])?;
    Ok(AsciiAdu { unit_id, pdu })
}

fn uppercase_hex(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    for b in data {
        out.extend_from_slice(format!("{:02X}", b).as_bytes());
    }
    out
}

fn hex_val(c: u8) -> Result<u8, ModbusError> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => Err(ModbusError::InvalidFrame(format!("bad hex char {}", c as char))),
    }
}

fn parse_hex(body: &[u8]) -> Result<Vec<u8>, ModbusError> {
    let mut out = Vec::with_capacity(body.len() / 2);
    for pair in body.chunks_exact(2) {
        out.push((hex_val(pair[0])? << 4) | hex_val(pair[1])?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l8_known_vector() {
        // 01 03 00 00 00 0A -> sum = 0x0E -> LRC = 0xF2
        assert_eq!(l8(&[0x01, 0x03, 0x00, 0x00, 0x00, 0x0A]), 0xF2);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let pdu = [0x03, 0x00, 0x00, 0x00, 0x0A];
        let frame = encode(UnitId(1), &pdu);
        let adu = decode(&frame).unwrap();
        assert_eq!(adu.unit_id, UnitId(1));
        assert_eq!(adu.pdu.to_bytes(), pdu.to_vec());
    }

    #[test]
    fn decode_bad_lrc() {
        let mut frame = encode(UnitId(1), &[0x03, 0x00]);
        // flip one hex char of the LRC (last two before CRLF)
        let idx = frame.len() - 3;
        frame[idx] = if frame[idx] == b'0' { b'1' } else { b'0' };
        assert!(matches!(decode(&frame), Err(ModbusError::LrcMismatch { .. })));
    }
}

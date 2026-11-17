//! Modbus RTU framing: binary ADU + CRC-16 (Modbus polynomial 0xA001).
//!
//! Frame layout:
//! `[slave id (1)] [function (1)] [data (0..253)] [CRC lo (1)] [CRC hi (1)]`
//!
//! Important for the scanner:
//! - inter-frame (silent interval) gap >= 3.5 char times (t3.5);
//! - CRC is transmitted low byte first;
//! - slave id 0 is broadcast (no response expected).

use super::common::{ModbusError, UnitId};
use super::frame::{Pdu, RtuAdu};

/// Maximum RTU ADU size (256 bytes incl. addresses + CRC).
pub const MAX_ADU_SIZE: usize = 256;

/// Compute Modbus CRC-16 over a byte slice.
/// Polynomial: 0xA001 (reversed 0x8005), init: 0xFFFF, no final xor.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= byte as u16;
        for _ in 0..8 {
            if crc & 0x0001 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

/// Build a full RTU frame (ADU with CRC) from a raw PDU byte buffer.
pub fn encode(unit_id: UnitId, pdu: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(1 + pdu.len() + 2);
    frame.push(unit_id.0);
    frame.extend_from_slice(pdu);
    let crc = crc16(&frame);
    // CRC is appended low byte first
    frame.push((crc & 0xFF) as u8);
    frame.push((crc >> 8) as u8);
    frame
}

/// Parse and verify a full RTU frame (with CRC).
pub fn decode(frame: &[u8]) -> Result<RtuAdu, ModbusError> {
    if frame.len() < 4 {
        return Err(ModbusError::TooShort(frame.len()));
    }
    let payload_len = frame.len() - 2;
    let (payload, crc_bytes) = frame.split_at(payload_len);
    let expected = u16::from_le_bytes([crc_bytes[0], crc_bytes[1]]);
    let actual = crc16(payload);
    if expected != actual {
        return Err(ModbusError::CrcMismatch { expected, actual });
    }
    let unit_id = UnitId(payload[0]);
    let pdu = Pdu::from_bytes(&payload[1..])?;
    Ok(RtuAdu { unit_id, pdu })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc16_known_vector() {
        // Classic test vector: 01 03 00 00 00 0A -> C5 CD (swapped on the wire)
        let data = [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A];
        assert_eq!(crc16(&data), 0xCDC5);
    }

    #[test]
    fn crc16_empty() {
        assert_eq!(crc16(&[]), 0xFFFF);
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
    fn decode_bad_crc() {
        let mut frame = encode(UnitId(1), &[0x03, 0x00]);
        *frame.last_mut().unwrap() ^= 0xFF;
        assert!(matches!(decode(&frame), Err(ModbusError::CrcMismatch { .. })));
    }
}

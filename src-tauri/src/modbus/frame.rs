//! ADU/PDU structures and function codes.
//!
//! - PDU (Protocol Data Unit): `[function code (1)] + [data (0..253)]`
//! - RTU ADU:   `[slave id (1)] + PDU + [CRC-16 (2)]`  (max 256 bytes)
//! - ASCII ADU: `':' + [slave id + PDU as hex] + [LRC (1)] + CRLF`
//! - TCP ADU:   MBAP header `[tx id (2)][protocol=0 (2)][length (2)][unit id (1)]` + PDU

use ts_rs::TS;
use serde::{Deserialize, Serialize};

use super::common::{ModbusError, Quantity, UnitId};

/// Supported Modbus function codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[repr(u8)]
pub enum FunctionCode {
    /// Read Coils (0x01)
    ReadCoils = 0x01,
    /// Read Discrete Inputs (0x02)
    ReadDiscreteInputs = 0x02,
    /// Read Holding Registers (0x03)
    ReadHoldingRegisters = 0x03,
    /// Read Input Registers (0x04)
    ReadInputRegisters = 0x04,
    /// Write Single Coil (0x05)
    WriteSingleCoil = 0x05,
    /// Write Single Register (0x06)
    WriteSingleRegister = 0x06,
    /// Write Multiple Coils (0x0F)
    WriteMultipleCoils = 0x0F,
    /// Write Multiple Registers (0x10)
    WriteMultipleRegisters = 0x10,
    /// Read/Write Multiple Registers (0x17)
    ReadWriteMultipleRegisters = 0x17,
    /// Mask Write Register (0x16)
    MaskWriteRegister = 0x16,
    /// Read Device Identification (0x2B / MEI 0x0E)
    ReadDeviceIdentification = 0x2B,
}

impl FunctionCode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::ReadCoils),
            0x02 => Some(Self::ReadDiscreteInputs),
            0x03 => Some(Self::ReadHoldingRegisters),
            0x04 => Some(Self::ReadInputRegisters),
            0x05 => Some(Self::WriteSingleCoil),
            0x06 => Some(Self::WriteSingleRegister),
            0x0F => Some(Self::WriteMultipleCoils),
            0x10 => Some(Self::WriteMultipleRegisters),
            0x16 => Some(Self::MaskWriteRegister),
            0x17 => Some(Self::ReadWriteMultipleRegisters),
            0x2B => Some(Self::ReadDeviceIdentification),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Maximum data quantity for this function (spec limits).
    pub fn max_quantity(self) -> Quantity {
        match self {
            FunctionCode::ReadCoils | FunctionCode::ReadDiscreteInputs => Quantity(2000),
            FunctionCode::ReadHoldingRegisters | FunctionCode::ReadInputRegisters => Quantity(125),
            FunctionCode::WriteMultipleCoils => Quantity(1968),
            FunctionCode::WriteMultipleRegisters => Quantity(123),
            _ => Quantity(1),
        }
    }
}

/// Application Data Unit variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Adu {
    /// Serial RTU frame: raw bytes incl. slave id + CRC.
    Rtu(RtuAdu),
    /// Serial ASCII frame: decoded payload (slave id + PDU + LRC), no start/end marks.
    Ascii(AsciiAdu),
    /// TCP frame: MBAP header + PDU.
    Tcp(TcpAdu),
}

/// RTU ADU (without CRC, CRC is computed/verified in `rtu` module).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtuAdu {
    pub unit_id: UnitId,
    pub pdu: Pdu,
}

/// ASCII ADU (without LRC and ':' / CRLF marks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsciiAdu {
    pub unit_id: UnitId,
    pub pdu: Pdu,
}

/// TCP/UDP ADU with MBAP header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpAdu {
    /// Transaction id (echoed by server)
    pub transaction_id: u16,
    /// Protocol id, always 0 for Modbus
    pub protocol_id: u16,
    pub unit_id: UnitId,
    pub pdu: Pdu,
}

/// Protocol Data Unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pdu {
    /// Request with function code + raw data (after the function byte).
    Request {
        function: u8,
        data: Vec<u8>,
    },
    /// Normal response.
    Response {
        function: u8,
        data: Vec<u8>,
    },
    /// Exception response: function | 0x80, exception code.
    Exception {
        function: u8,
        code: u8,
    },
}

impl Pdu {
    pub fn function(&self) -> u8 {
        match self {
            Pdu::Request { function, .. } => *function,
            Pdu::Response { function, .. } => *function,
            Pdu::Exception { function, .. } => *function,
        }
    }

    pub fn is_exception(&self) -> bool {
        matches!(self, Pdu::Exception { .. })
    }

    /// Wire frames do not encode their direction. RTU/ASCII decoders and
    /// from_bytes return a generic Request for regular frames; consumers
    /// verifying a received reply must explicitly tag it as Response.
    pub fn into_response(self) -> Self {
        match self {
            Pdu::Request { function, data } => Pdu::Response { function, data },
            other => other,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Pdu::Request { function, data } => {
                let mut v = Vec::with_capacity(1 + data.len());
                v.push(*function);
                v.extend_from_slice(data);
                v
            }
            Pdu::Response { function, data } => {
                let mut v = Vec::with_capacity(1 + data.len());
                v.push(*function);
                v.extend_from_slice(data);
                v
            }
            Pdu::Exception { function, code } => vec![function | 0x80, *code],
        }
    }

    /// Parse a PDU from raw bytes (min 1 byte).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ModbusError> {
        if bytes.is_empty() {
            return Err(ModbusError::TooShort(0));
        }
        let function = bytes[0];
        if function & 0x80 != 0 {
            if bytes.len() < 2 {
                return Err(ModbusError::TooShort(bytes.len()));
            }
            return Ok(Pdu::Exception {
                function: function & 0x7F,
                code: bytes[1],
            });
        }
        Ok(Pdu::Request {
            function,
            data: bytes[1..].to_vec(),
        })
    }
}

#[cfg(test)]
mod response_direction_tests {
    use super::*;

    #[test]
    fn normal_wire_pdu_is_tagged_as_response_at_receive_boundary() {
        let raw = [3, 2, 0, 42];
        let pdu = Pdu::from_bytes(&raw).unwrap();
        assert!(matches!(pdu, Pdu::Request { .. }));
        let response = pdu.into_response();
        assert!(matches!(response, Pdu::Response { function: 3, data } if data == [2, 0, 42]));
        assert_eq!(Pdu::from_bytes(&[0x83, 0x02]).unwrap().into_response(),
                   Pdu::Exception { function: 3, code: 2 });
    }
}

/// One identification object returned by FC43/MEI14.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DeviceIdObject {
    pub id: u8,
    pub value: String,
}

/// Parsed Read Device Identification response (FC43 / MEI 0x0E).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DeviceIdentification {
    /// Requested category: 1 basic, 2 regular, 3 extended, 4 individual object.
    pub read_code: u8,
    pub conformity_level: u8,
    pub more_follows: bool,
    pub next_object_id: u8,
    pub objects: Vec<DeviceIdObject>,
}

impl DeviceIdentification {
    pub fn get(&self, id: u8) -> Option<&str> {
        self.objects.iter().find(|o| o.id == id).map(|o| o.value.as_str())
    }

    /// Standard object 0 — vendor name.
    pub fn vendor_name(&self) -> Option<&str> {
        self.get(0)
    }

    /// Standard object 1 — product code.
    pub fn product_code(&self) -> Option<&str> {
        self.get(1)
    }

    /// Standard object 2 — major/minor revision.
    pub fn revision(&self) -> Option<&str> {
        self.get(2)
    }
}

/// Build a basic device-identification request (FC43, MEI 0x0E, category 1).
pub fn read_device_identification(read_code: u8, object_id: u8) -> Vec<u8> {
    vec![0x2B, 0x0E, read_code, object_id]
}

/// Parse the PDU data of an FC43 response (starts with MEI type 0x0E).
pub fn parse_device_identification(data: &[u8]) -> Result<DeviceIdentification, ModbusError> {
    if data.len() < 6 {
        return Err(ModbusError::TooShort(data.len()));
    }
    if data[0] != 0x0E {
        return Err(ModbusError::InvalidFrame(format!(
            "unexpected MEI type {:#04x}, expected 0x0E",
            data[0]
        )));
    }
    let read_code = data[1];
    let conformity_level = data[2];
    let more_follows = data[3] != 0;
    let next_object_id = data[4];
    let count = data[5] as usize;
    let mut pos = 6usize;
    let mut objects = Vec::with_capacity(count);
    for _ in 0..count {
        if pos + 2 > data.len() {
            return Err(ModbusError::TooShort(data.len()));
        }
        let id = data[pos];
        let len = data[pos + 1] as usize;
        pos += 2;
        if pos + len > data.len() {
            return Err(ModbusError::TooShort(data.len()));
        }
        objects.push(DeviceIdObject {
            id,
            value: String::from_utf8_lossy(&data[pos..pos + len]).into_owned(),
        });
        pos += len;
    }
    Ok(DeviceIdentification {
        read_code,
        conformity_level,
        more_follows,
        next_object_id,
        objects,
    })
}

#[cfg(test)]
mod device_id_tests {
    use super::*;

    #[test]
    fn parse_basic_identification() {
        // MEI 0x0E, basic, conformity 0x01, no continuation, next id 0,
        // 3 objects: 0="VEM", 1="PL", 2="1"
        let data = [
            0x0E, 0x01, 0x01, 0x00, 0x00, 0x03, 0x00, 0x03, b'V', b'E', b'M', 0x01, 0x02, b'P',
            b'L', 0x02, 0x01, b'1',
        ];
        let id = parse_device_identification(&data).unwrap();
        assert_eq!(id.vendor_name(), Some("VEM"));
        assert_eq!(id.product_code(), Some("PL"));
        assert_eq!(id.revision(), Some("1"));
        assert!(!id.more_follows);
    }

    #[test]
    fn request_pdu() {
        assert_eq!(read_device_identification(1, 0), vec![0x2B, 0x0E, 0x01, 0x00]);
    }
}

/// Request builders for read/write function codes (PDU level).
pub mod request {

    fn read_build(function: u8, addr: u16, qty: u16) -> Vec<u8> {
        let mut d = Vec::with_capacity(4);
        d.extend_from_slice(&addr.to_be_bytes());
        d.extend_from_slice(&qty.to_be_bytes());
        let mut v = vec![function];
        v.extend_from_slice(&d);
        v
    }

    pub fn read_coils(addr: u16, qty: u16) -> Vec<u8> {
        read_build(0x01, addr, qty)
    }
    pub fn read_discrete_inputs(addr: u16, qty: u16) -> Vec<u8> {
        read_build(0x02, addr, qty)
    }
    pub fn read_holding_registers(addr: u16, qty: u16) -> Vec<u8> {
        read_build(0x03, addr, qty)
    }
    pub fn read_input_registers(addr: u16, qty: u16) -> Vec<u8> {
        read_build(0x04, addr, qty)
    }

    pub fn write_single_coil(addr: u16, on: bool) -> Vec<u8> {
        let mut v = vec![0x05];
        v.extend_from_slice(&addr.to_be_bytes());
        v.extend_from_slice(&if on { [0xFF, 0x00] } else { [0x00, 0x00] });
        v
    }

    pub fn write_single_register(addr: u16, value: u16) -> Vec<u8> {
        let mut v = vec![0x06];
        v.extend_from_slice(&addr.to_be_bytes());
        v.extend_from_slice(&value.to_be_bytes());
        v
    }

    pub fn write_multiple_registers(addr: u16, values: &[u16]) -> Vec<u8> {
        let mut v = vec![0x10];
        v.extend_from_slice(&addr.to_be_bytes());
        v.extend_from_slice(&(values.len() as u16).to_be_bytes());
        v.push((values.len() * 2) as u8);
        for val in values {
            v.extend_from_slice(&val.to_be_bytes());
        }
        v
    }
}

/// Response data parsers (PDU level).
pub mod response {
    use super::super::common::ModbusError;

    /// Parse a read-coils/discrete-inputs response into a bit vector.
    pub fn parse_bits(data: &[u8], expected: u16) -> Result<Vec<bool>, ModbusError> {
        // data: [byte count][bytes...]
        if data.is_empty() {
            return Err(ModbusError::TooShort(0));
        }
        let byte_count = data[0] as usize;
        if data.len() < 1 + byte_count {
            return Err(ModbusError::InvalidFrame(format!(
                "byte count {} > payload {}",
                byte_count,
                data.len() - 1
            )));
        }
        let mut bits = Vec::with_capacity(expected as usize);
        for i in 0..expected as usize {
            let byte = data[1 + i / 8];
            bits.push((byte >> (i % 8)) & 0x01 != 0);
        }
        Ok(bits)
    }

    /// Parse a read-registers response into a u16 vector.
    pub fn parse_registers(data: &[u8]) -> Result<Vec<u16>, ModbusError> {
        if data.is_empty() {
            return Err(ModbusError::TooShort(0));
        }
        let byte_count = data[0] as usize;
        if byte_count % 2 != 0 || data.len() < 1 + byte_count {
            return Err(ModbusError::InvalidFrame(format!(
                "bad byte count {} / payload {}",
                byte_count,
                data.len() - 1
            )));
        }
        Ok(data[1..1 + byte_count]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect())
    }
}

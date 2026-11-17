//! ADU/PDU structures and function codes.
//!
//! - PDU (Protocol Data Unit): `[function code (1)] + [data (0..253)]`
//! - RTU ADU:   `[slave id (1)] + PDU + [CRC-16 (2)]`  (max 256 bytes)
//! - ASCII ADU: `':' + [slave id + PDU as hex] + [LRC (1)] + CRLF`
//! - TCP ADU:   MBAP header `[tx id (2)][protocol=0 (2)][length (2)][unit id (1)]` + PDU

use serde::{Deserialize, Serialize};

use super::common::{ModbusError, Quantity, UnitId};

/// Supported Modbus function codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

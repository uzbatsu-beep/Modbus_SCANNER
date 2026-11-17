//! Shared protocol types: UnitId, Address, Quantity, protocol errors.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Modbus unit (slave) identifier. RTU: 0-247 (0 = broadcast), TCP: 0-255.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnitId(pub u8);

/// Register/coil address (0..=65535). Note: in the PDU the address is 0-based,
/// in "Modbus data model" notation (1..=65536) it is shifted by 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address(pub u16);

/// Quantity of coils/registers to read/write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quantity(pub u16);

/// Modbus transport kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportKind {
    /// Modbus RTU over serial (binary + CRC-16)
    Rtu,
    /// Modbus ASCII over serial (hex + LRC)
    Ascii,
    /// Modbus TCP
    Tcp,
    /// Modbus UDP (RTU-like ADU over UDP, non-standard but used in the field)
    Udp,
}

/// Serial framing mode (RTU vs ASCII).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialMode {
    /// Binary frames + CRC-16.
    Rtu,
    /// Hex-encoded frames + LRC.
    Ascii,
}

/// Modbus protocol-level errors.
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModbusError {
    /// Illegal function code (exception 0x01)
    #[error("illegal function (0x01)")]
    IllegalFunction,
    /// Illegal data address (exception 0x02)
    #[error("illegal data address (0x02)")]
    IllegalDataAddress,
    /// Illegal data value (exception 0x03)
    #[error("illegal data value (0x03)")]
    IllegalDataValue,
    /// Slave device failure (exception 0x04)
    #[error("slave device failure (0x04)")]
    SlaveDeviceFailure,
    /// Acknowledge (exception 0x05)
    #[error("acknowledge (0x05)")]
    Acknowledge,
    /// Slave device busy (exception 0x06)
    #[error("slave device busy (0x06)")]
    SlaveDeviceBusy,
    /// Negative acknowledge (exception 0x07)
    #[error("negative acknowledge (0x07)")]
    NegativeAcknowledge,
    /// Memory parity error (exception 0x08)
    #[error("memory parity error (0x08)")]
    MemoryParityError,
    /// Gateway path unavailable (exception 0x0A)
    #[error("gateway path unavailable (0x0A)")]
    GatewayPathUnavailable,
    /// Gateway target failed to respond (exception 0x0B)
    #[error("gateway target failed to respond (0x0B)")]
    GatewayTargetDeviceFailed,
    /// CRC checksum mismatch (RTU)
    #[error("CRC mismatch: expected {expected:#06x}, got {actual:#06x}")]
    CrcMismatch {
        expected: u16,
        actual: u16,
    },
    /// LRC checksum mismatch (ASCII)
    #[error("LRC mismatch: expected {expected:#04x}, got {actual:#04x}")]
    LrcMismatch {
        expected: u8,
        actual: u8,
    },
    /// Invalid frame structure / length
    #[error("invalid frame: {0}")]
    InvalidFrame(String),
    /// Invalid unit id in response (does not match request)
    #[error("unit id mismatch: expected {expected}, got {actual}")]
    UnitIdMismatch {
        expected: u8,
        actual: u8,
    },
    /// Invalid transaction id (TCP/UDP)
    #[error("transaction id mismatch: expected {expected}, got {actual}")]
    TransactionIdMismatch {
        expected: u16,
        actual: u16,
    },
    /// Unexpected function code in response
    #[error("unexpected function code: {0:#04x}")]
    UnexpectedFunction(u8),
    /// I/O or transport-level error (timeout, closed, etc.)
    #[error("io error: {0}")]
    Io(String),
    /// Response too short
    #[error("response too short: {0} bytes")]
    TooShort(usize),
}

impl ModbusError {
    /// Build a protocol exception error from an exception code (0x01..=0x0B).
    pub fn from_exception(code: u8) -> Self {
        match code {
            0x01 => ModbusError::IllegalFunction,
            0x02 => ModbusError::IllegalDataAddress,
            0x03 => ModbusError::IllegalDataValue,
            0x04 => ModbusError::SlaveDeviceFailure,
            0x05 => ModbusError::Acknowledge,
            0x06 => ModbusError::SlaveDeviceBusy,
            0x07 => ModbusError::NegativeAcknowledge,
            0x08 => ModbusError::MemoryParityError,
            0x0A => ModbusError::GatewayPathUnavailable,
            0x0B => ModbusError::GatewayTargetDeviceFailed,
            other => ModbusError::InvalidFrame(format!("unknown exception code {other:#04x}")),
        }
    }
}

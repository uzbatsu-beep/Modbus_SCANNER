//! Modbus protocol implementation.
//!
//! - `frame`   - ADU/PDU structures, function codes, exceptions
//! - `rtu`     - RTU framing (8-bit binary + CRC-16)
//! - `ascii`   - ASCII framing (hex-encoding + LRC)
//! - `common`  - shared types (UnitId, Address, Quantity, errors)

pub mod ascii;
pub mod common;
pub mod decode;
pub mod frame;
pub mod rtu;

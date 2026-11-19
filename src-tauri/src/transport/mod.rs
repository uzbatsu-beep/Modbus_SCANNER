//! Async transports for Modbus.
//!
//! - `serial` - RTU/ASCII over a serial port (tokio-serial)
//! - `tcp`    - Modbus TCP client (tokio TcpStream)
//! - `udp`    - Modbus UDP client (tokio UdpSocket)
//!
//! All transports implement the same trait [`Transport`]: the scanner and the
//! protocol codec work with a uniform async request/response interface, so any
//! combination (RTU/ASCII codec over serial, RTU-over-TCP "Modbus RTU over TCP"
//! or "Modbus UDP") can be composed freely.

use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;

use crate::modbus::common::ModbusError;

/// Errors at the transport level.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("timeout after {0:?}")]
    Timeout(Duration),
    #[error("port not open: {0}")]
    NotOpen(String),
    #[error("connection closed")]
    Closed,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("modbus: {0}")]
    Modbus(#[from] ModbusError),
    #[error("operation cancelled")]
    Cancelled,
    #[error("transport-specific: {0}")]
    Other(String),
}

/// Result alias for transport operations.
pub type TransportResult<T> = Result<T, TransportError>;

/// Abstract async Modbus transport.
///
/// One `send_raw` / `recv_raw` roundtrip corresponds to one ADU.
/// The transport is responsible for:
/// - writing the raw ADU bytes;
/// - reading back the response ADU bytes (with timeout);
/// - serial transports: enforcing the inter-frame (silent) interval t3.5;
/// - TCP/UDP transports: MBAP length framing.
#[async_trait]
pub trait Transport: Send {
    /// Transport display name (for logs).
    fn name(&self) -> String;

    /// Flush the underlying buffers (and clear stale RX data).
    async fn reset(&mut self) -> TransportResult<()>;

    /// Close the transport (idempotent).
    async fn close(&mut self) -> TransportResult<()>;

    /// Write a raw ADU frame.
    async fn send_raw(&mut self, adu: &[u8]) -> TransportResult<()>;

    /// Read one raw response ADU frame (bounded by transport-specific framing).
    async fn recv_raw(&mut self, expected_hint: usize) -> TransportResult<Vec<u8>>;

    /// Convenience: send + recv with the given response timeout.
    async fn transact(
        &mut self,
        adu: &[u8],
        response_timeout: Duration,
        expected_hint: usize,
    ) -> TransportResult<Vec<u8>> {
        self.reset().await?;
        self.send_raw(adu).await?;
        match tokio::time::timeout(response_timeout, self.recv_raw(expected_hint)).await {
            Ok(res) => res,
            Err(_) => Err(TransportError::Timeout(response_timeout)),
        }
    }
}

#[cfg(test)]
pub mod mock;
pub mod serial;
pub mod tcp;
pub mod tls;
pub mod udp;

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

use std::time::{Duration, Instant};
use timing::{micros, TransactionTiming};
pub mod timing;
pub mod connection;

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
        self.transact_measured(adu, response_timeout, expected_hint, &mut TransactionTiming::default()).await
    }

    /// Same reset/send/receive sequence, with measured stages and completed ADU counts.
    async fn transact_measured(
        &mut self, adu: &[u8], response_timeout: Duration, expected_hint: usize,
        timing: &mut TransactionTiming,
    ) -> TransportResult<Vec<u8>> {
        self.transact_guarded(adu,response_timeout,expected_hint,timing,&mut || true).await
    }

    /// The worker supplies a synchronous admission gate after reset and before
    /// TX. Once admitted, the bounded TX/RX sequence finishes without cancellation.
    async fn transact_guarded(
        &mut self, adu:&[u8], response_timeout:Duration, expected_hint:usize,
        timing:&mut TransactionTiming, admit_tx:&mut (dyn FnMut()->bool+Send),
    )->TransportResult<Vec<u8>>{
        let start = Instant::now();
        let reset = self.reset().await;
        timing.reset_us = Some(micros(start));
        if let Err(e) = reset { timing.failure_stage = Some("reset".into()); return Err(e); }
        if !admit_tx(){timing.failure_stage=Some("pre_tx_cancelled".into());return Err(TransportError::Cancelled);}
        let tx = Instant::now();
        let sent = self.send_raw(adu).await;
        timing.tx_us = Some(micros(tx));
        if let Err(e) = sent { timing.failure_stage = Some("tx".into()); return Err(e); }
        timing.tx_complete = true;
        timing.tx_bytes = adu.len().min(u32::MAX as usize) as u32;
        let rx = Instant::now();
        let result = match tokio::time::timeout(response_timeout, self.recv_raw(expected_hint)).await {
            Ok(res) => res,
            Err(_) => Err(TransportError::Timeout(response_timeout)),
        };
        timing.rx_us = Some(micros(rx));
        match &result {
            Ok(bytes) => {
                timing.rtt_us = Some(micros(tx));
                timing.rx_complete = true;
                timing.rx_bytes = bytes.len().min(u32::MAX as usize) as u32;
            }
            Err(_) => timing.failure_stage = Some("rx".into()),
        }
        result
    }
}

#[cfg(test)]
pub mod mock;
pub mod serial;
pub mod tcp;
pub mod tls;
pub mod udp;

#[cfg(test)]
mod timing_tests {
    use super::*;
    use mock::MockTransport;
    #[tokio::test]
    async fn measured_transaction_records_completed_frames_and_stage() {
        let mut t=MockTransport::new(vec![Ok(vec![1,2,3]),Err(TransportError::Timeout(Duration::from_millis(10)))]);
        let mut m=TransactionTiming::default();
        assert!(t.transact_measured(&[1,3],Duration::from_millis(20),3,&mut m).await.is_ok());
        assert!(m.tx_complete && m.rx_complete);
        assert_eq!((m.tx_bytes,m.rx_bytes),(2,3));
        assert!(m.rtt_us.is_some() && m.connect_us.is_none());
        let mut m=TransactionTiming::default();
        assert!(t.transact_measured(&[1,3],Duration::from_millis(20),3,&mut m).await.is_err());
        assert!(m.tx_complete && !m.rx_complete);
        assert!(m.rtt_us.is_none());
        assert_eq!(m.failure_stage.as_deref(),Some("rx"));
    }
}

//! Modbus TCP transport.
//!
//! MBAP header (7 bytes, big endian):
//! `[transaction id (2)] [protocol id = 0 (2)] [length (2)] [unit id (1)]`
//! `length` = number of bytes following it (unit id + PDU), i.e. 1 + pdu_len.
//!
//! Response matching:
//! - transaction id must be echoed;
//! - protocol id must be 0;
//! - unit id is checked at the protocol layer (gateways may rewrite it).

use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::{Transport, TransportError, TransportResult};

/// TCP connection parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TcpConfig {
    pub host: String,
    /// Modbus TCP default port 502; some devices use 503, 1502, 802...
    pub port: u16,
    /// TCP connect timeout.
    pub connect_timeout: Duration,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 502,
            connect_timeout: Duration::from_millis(2000),
        }
    }
}

/// Modbus TCP client transport.
pub struct TcpTransport {
    config: TcpConfig,
    stream: TcpStream,
    /// Incrementing transaction id source.
    next_transaction_id: u16,
}

impl TcpTransport {
    /// Connect to a Modbus TCP server.
    pub async fn connect(config: TcpConfig) -> TransportResult<Self> {
        let addr = format!("{}:{}", config.host, config.port);
        let stream = match tokio::time::timeout(
            config.connect_timeout,
            TcpStream::connect(&addr),
        )
        .await
        {
            Ok(res) => res?,
            Err(_) => return Err(TransportError::Timeout(config.connect_timeout)),
        };
        // TCP_NODELAY: important for low-latency scanning.
        stream.set_nodelay(true)?;
        Ok(Self {
            config,
            stream,
            next_transaction_id: rand::random(),
        })
    }

    /// Read one full MBAP-framed response.
    async fn read_mbap(&mut self) -> TransportResult<Vec<u8>> {
        let mut header = [0u8; 7];
        self.stream.read_exact(&mut header).await?;

        let transaction_id = u16::from_be_bytes([header[0], header[1]]);
        let protocol_id = u16::from_be_bytes([header[2], header[3]]);
        let length = u16::from_be_bytes([header[4], header[5]]);

        if protocol_id != 0 {
            return Err(TransportError::Other(format!(
                "bad protocol id {protocol_id}"
            )));
        }
        // length includes unit id + PDU; guard against nonsense values
        if !(1..=254).contains(&length) {
            return Err(TransportError::Other(format!("bad mbap length {length}")));
        }

        let mut rest = vec![0u8; length as usize];
        self.stream.read_exact(&mut rest).await?;

        let mut frame = header.to_vec();
        frame.extend_from_slice(&rest);
        // Stash transaction id for the caller via first 2 bytes of the frame.
        let _ = transaction_id;
        Ok(frame)
    }
}

#[async_trait]
impl Transport for TcpTransport {
    fn name(&self) -> String {
        format!("tcp://{}:{}", self.config.host, self.config.port)
    }

    async fn reset(&mut self) -> TransportResult<()> {
        Ok(()) // no stale data concept for a stream with MBAP framing
    }

    async fn close(&mut self) -> TransportResult<()> {
        let _ = self.stream.shutdown().await;
        Ok(())
    }

    async fn send_raw(&mut self, adu: &[u8]) -> TransportResult<()> {
        // If the caller passed a full MBAP frame (>= 7 bytes), send as is.
        // Otherwise (raw RTU ADU) — wrap into MBAP: "Modbus RTU over TCP" support.
        let out = if adu.len() >= 7 && is_mbap(adu) {
            adu.to_vec()
        } else {
            let transaction_id = self.next_transaction_id;
            self.next_transaction_id = self.next_transaction_id.wrapping_add(1);
            let unit_id = adu.first().copied().unwrap_or(1);
            let mut frame = Vec::with_capacity(7 + adu.len());
            frame.extend_from_slice(&transaction_id.to_be_bytes());
            frame.extend_from_slice(&0u16.to_be_bytes()); // protocol id
            frame.extend_from_slice(&((1 + adu.len().saturating_sub(1)) as u16).to_be_bytes());
            frame.push(unit_id);
            frame.extend_from_slice(&adu[1.min(adu.len())..]);
            frame
        };
        self.stream.write_all(&out).await?;
        self.stream.flush().await?;
        Ok(())
    }

    async fn recv_raw(&mut self, _expected_hint: usize) -> TransportResult<Vec<u8>> {
        self.read_mbap().await
    }
}

/// Heuristic: MBAP frame has protocol id == 0 at bytes 2..3.
fn is_mbap(adu: &[u8]) -> bool {
    adu.len() >= 8 && adu[2] == 0 && adu[3] == 0
}

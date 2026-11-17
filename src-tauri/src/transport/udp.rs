//! Modbus UDP transport.
//!
//! Modbus over UDP is not in the original spec but is widely used in the field:
//! the payload is either a full MBAP frame (TCP-style, most common) or an
//! RTU-style ADU. One request datagram -> one response datagram.
//!
//! Scanner notes:
//! - UDP is connectionless: "no response" (timeout) is the normal negative result;
//! - a shorter RX timeout is acceptable for high-speed IP scanning;
//! - send-to / recv-from with the same remote endpoint.

use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;
use tokio::net::UdpSocket;

use super::{Transport, TransportError, TransportResult};

/// UDP parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UdpConfig {
    pub host: String,
    /// Modbus UDP port: 502 by convention.
    pub port: u16,
    /// Bind timeout (local socket creation).
    pub bind_timeout: Duration,
}

impl Default for UdpConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 502,
            bind_timeout: Duration::from_millis(1000),
        }
    }
}

/// Modbus UDP client transport.
pub struct UdpTransport {
    remote: SocketAddr,
    socket: UdpSocket,
    /// Buffer for one datagram (max Modbus ADU + slack).
    rx_buf: Vec<u8>,
}

impl UdpTransport {
    /// Create a UDP "connection" (local ephemeral port -> remote endpoint).
    pub async fn connect(host: &str, port: u16) -> TransportResult<Self> {
        let remote: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|e| TransportError::Other(format!("bad address {host}:{port}: {e}")))?;
        let local: SocketAddr = if remote.is_ipv4() {
            "0.0.0.0:0".parse().unwrap()
        } else {
            "[::]:0".parse().unwrap()
        };
        let socket = UdpSocket::bind(local).await?;
        // Connect the socket so we only receive datagrams from the remote.
        socket.connect(remote).await?;
        Ok(Self {
            remote,
            socket,
            rx_buf: vec![0u8; 1024],
        })
    }
}

#[async_trait]
impl Transport for UdpTransport {
    fn name(&self) -> String {
        format!("udp://{}", self.remote)
    }

    async fn reset(&mut self) -> TransportResult<()> {
        Ok(()) // datagram-oriented: no stale partial reads
    }

    async fn close(&mut self) -> TransportResult<()> {
        Ok(()) // UdpSocket closes on drop
    }

    async fn send_raw(&mut self, adu: &[u8]) -> TransportResult<()> {
        self.socket.send(adu).await?;
        Ok(())
    }

    async fn recv_raw(&mut self, _expected_hint: usize) -> TransportResult<Vec<u8>> {
        // Read one full datagram.
        let (n, _peer) = self.socket.recv_from(&mut self.rx_buf).await?;
        Ok(self.rx_buf[..n].to_vec())
    }
}

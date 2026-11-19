//! Serial port transport (Modbus RTU/ASCII).
//!
//! Implements byte-level I/O over a [`tokio_serial::SerialStream`] with:
//! - configurable baud/data bits/parity/stop bits;
//! - inter-frame silent interval (t3.5) enforcement;
//! - RX buffer pre-flush on each transaction (stale data discard).

use ts_rs::TS;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serial::{DataBits, Parity, SerialStream, StopBits};

use crate::modbus::common::SerialMode;

use super::{Transport, TransportError, TransportResult};

/// Serial port parameters.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SerialPortConfig {
    /// Port name, e.g. "COM3" (Windows) or "/dev/ttyUSB0" (Linux).
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: DataBitsSerde,
    pub parity: ParitySerde,
    pub stop_bits: StopBitsSerde,
    /// Inter-frame silent interval enforcement mode (RTU t3.5).
    /// On modern baud rates (>= 19200) the spec fixes t3.5 = 1.75 ms.
    #[ts(type = "number")]
    pub frame_gap: Duration,
}

/// Serializable serial enum wrappers (tokio_serial enums are not Serialize).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum DataBitsSerde {
    Five,
    Six,
    Seven,
    Eight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ParitySerde {
    None,
    Odd,
    Even,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum StopBitsSerde {
    One,
    Two,
}

impl From<ParitySerde> for Parity {
    fn from(p: ParitySerde) -> Self {
        match p {
            ParitySerde::None => Parity::None,
            ParitySerde::Odd => Parity::Odd,
            ParitySerde::Even => Parity::Even,
        }
    }
}

impl From<DataBitsSerde> for DataBits {
    fn from(d: DataBitsSerde) -> Self {
        match d {
            DataBitsSerde::Five => DataBits::Five,
            DataBitsSerde::Six => DataBits::Six,
            DataBitsSerde::Seven => DataBits::Seven,
            DataBitsSerde::Eight => DataBits::Eight,
        }
    }
}

impl From<StopBitsSerde> for StopBits {
    fn from(s: StopBitsSerde) -> Self {
        match s {
            StopBitsSerde::One => StopBits::One,
            StopBitsSerde::Two => StopBits::Two,
        }
    }
}

impl Default for SerialPortConfig {
    fn default() -> Self {
        Self {
            port_name: String::new(),
            baud_rate: 9600,
            data_bits: DataBitsSerde::Eight,
            parity: ParitySerde::None,
            stop_bits: StopBitsSerde::One,
            frame_gap: Duration::from_millis(2),
        }
    }
}

/// Modbus RTU/ASCII serial transport.
pub struct SerialTransport {
    port_name: String,
    config: SerialPortConfig,
    stream: SerialStream,
    /// RX buffer for partially received frames.
    rx_buf: Vec<u8>,
    /// Common baud presets (RTU): 9600 8N1 / 8E1 / 8O1, 19200..115200.
    mode: SerialMode,
}

impl SerialTransport {
    /// Open a serial port with the given parameters.
    pub async fn open(config: SerialPortConfig, mode: SerialMode) -> TransportResult<Self> {
        let builder = tokio_serial::new(&config.port_name, config.baud_rate)
            .data_bits(config.data_bits.into())
            .parity(config.parity.into())
            .stop_bits(config.stop_bits.into());

        let stream = tokio_serial::SerialStream::open(&builder)
            .map_err(|e| TransportError::NotOpen(format!("{}: {e}", config.port_name)))?;

        Ok(Self {
            port_name: config.port_name.clone(),
            config,
            stream,
            rx_buf: Vec::with_capacity(512),
            mode,
        })
    }

    /// Read raw bytes until a frame end condition is met:
    /// - RTU: no fixed terminator — use the silent interval (no new bytes for N ms);
    /// - ASCII: read until CRLF (b'\r' b'\n').
    async fn read_frame_raw(&mut self, quiet_ms: u64) -> TransportResult<Vec<u8>> {
        let quiet = Duration::from_millis(quiet_ms);
        let mut tmp = [0u8; 512];

        loop {
            let wait = tokio::time::timeout(quiet, self.stream.read(&mut tmp)).await;
            match wait {
                Err(_) => {
                    // silent interval elapsed — RTU frame complete (or nothing came)
                    if !self.rx_buf.is_empty() {
                        let frame = std::mem::take(&mut self.rx_buf);
                        return Ok(frame);
                    }
                    return Err(TransportError::Timeout(quiet));
                }
                Ok(Err(e)) => return Err(TransportError::Io(e)),
                Ok(Ok(0)) => return Err(TransportError::Closed),
                Ok(Ok(n)) => {
                    self.rx_buf.extend_from_slice(&tmp[..n]);
                    if self.mode == SerialMode::Ascii {
                        // ASCII: check for CRLF terminator
                        if self.rx_buf.ends_with(b"\r\n") && self.rx_buf.len() >= 5 {
                            let frame = std::mem::take(&mut self.rx_buf);
                            return Ok(frame);
                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
impl Transport for SerialTransport {
    fn name(&self) -> String {
        format!(
            "{}@{} ({} mode)",
            self.port_name,
            self.config.baud_rate,
            match self.mode {
                SerialMode::Rtu => "RTU",
                SerialMode::Ascii => "ASCII",
            }
        )
    }

    async fn reset(&mut self) -> TransportResult<()> {
        // Discard any stale RX data before a new transaction.
        self.rx_buf.clear();
        let mut tmp = [0u8; 512];
        loop {
            match tokio::time::timeout(Duration::from_millis(5), self.stream.read(&mut tmp)).await
            {
                Ok(Ok(n)) if n > 0 => continue,
                Ok(_) => break,
                Err(_) => break,
            }
        }
        Ok(())
    }

    async fn close(&mut self) -> TransportResult<()> {
        self.stream.flush().await?;
        Ok(())
    }

    async fn send_raw(&mut self, adu: &[u8]) -> TransportResult<()> {
        // Enforce inter-frame gap (t3.5) before transmitting.
        if self.mode == SerialMode::Rtu {
            tokio::time::sleep(self.config.frame_gap).await;
        }
        self.stream.write_all(adu).await?;
        self.stream.flush().await?;
        Ok(())
    }

    async fn recv_raw(&mut self, _expected_hint: usize) -> TransportResult<Vec<u8>> {
        // RTU: 5 ms of silence = frame end (scanner-oriented, works well up to 115200).
        // ASCII: CRLF terminator.
        let quiet_ms = match self.mode {
            SerialMode::Rtu => 5,
            SerialMode::Ascii => 5,
        };
        self.read_frame_raw(quiet_ms).await
    }
}

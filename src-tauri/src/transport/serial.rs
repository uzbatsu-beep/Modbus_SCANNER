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
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio_serial::{DataBits, Parity, SerialStream, StopBits};

use crate::modbus::common::{ModbusError, SerialMode};

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

/// Modbus Serial Line specification: at <=19200 baud t3.5 is 3.5 character times;
/// above 19200 baud the recommended fixed delay is 1.75 ms.
pub fn required_rtu_gap(baud_rate: u32, data_bits: DataBitsSerde, parity: ParitySerde, stop_bits: StopBitsSerde) -> Duration {
    if baud_rate > 19_200 {
        return Duration::from_micros(1_750);
    }
    let data = match data_bits {
        DataBitsSerde::Five => 5u64,
        DataBitsSerde::Six => 6,
        DataBitsSerde::Seven => 7,
        DataBitsSerde::Eight => 8,
    };
    let parity_bit = if parity == ParitySerde::None { 0 } else { 1 };
    let stop = if stop_bits == StopBitsSerde::Two { 2 } else { 1 };
    let bits_per_character = 1u64 + data + parity_bit + stop;
    let baud = u64::from(baud_rate.max(1));
    Duration::from_micros((bits_per_character * 3_500_000).div_ceil(baud))
}

impl SerialTransport {
    /// Open a serial port with the given parameters.
    pub async fn open(mut config: SerialPortConfig, mode: SerialMode) -> TransportResult<Self> {
        if mode == SerialMode::Rtu {
            config.frame_gap = config.frame_gap.max(required_rtu_gap(
                config.baud_rate, config.data_bits, config.parity, config.stop_bits,
            ));
        }
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
    async fn read_frame_raw(&mut self, quiet: Duration) -> TransportResult<Vec<u8>> {
        read_frame_from(&mut self.stream, &mut self.rx_buf, self.mode, quiet).await
    }
}

/// The generic reader makes delayed-first-byte and fragmented-ASCII cases
/// testable without physical RS-485 hardware.
async fn read_frame_from<R: AsyncRead + Unpin>(
    stream: &mut R,
    rx_buf: &mut Vec<u8>,
    mode: SerialMode,
    quiet: Duration,
) -> TransportResult<Vec<u8>> {
    let mut tmp = [0u8; 512];
    loop {
        match tokio::time::timeout(quiet, stream.read(&mut tmp)).await {
            Err(_) => {
                // Before the first byte the outer request timeout controls the wait.
                // A partial ASCII frame must wait for CRLF, not a short quiet gap.
                if mode == SerialMode::Rtu && !rx_buf.is_empty() {
                    return Ok(std::mem::take(rx_buf));
                }
            }
            Ok(Err(e)) => return Err(TransportError::Io(e)),
            Ok(Ok(0)) => return Err(TransportError::Closed),
            Ok(Ok(n)) => {
                rx_buf.extend_from_slice(&tmp[..n]);
                // RTU ADU <= 256 bytes. ASCII carries up to 255 binary
                // payload bytes as 510 hex chars plus ':' and CRLF (513).
                // A noisy or unterminated bus must not grow RX indefinitely.
                let limit = match mode {
                    SerialMode::Rtu => 256,
                    SerialMode::Ascii => 513,
                };
                if rx_buf.len() > limit {
                    rx_buf.clear();
                    return Err(TransportError::Modbus(ModbusError::InvalidFrame(
                        format!("serial response exceeds {limit} bytes"),
                    )));
                }
                if mode == SerialMode::Ascii && rx_buf.len() >= 5 && rx_buf.ends_with(b"\r\n") {
                    return Ok(std::mem::take(rx_buf));
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
        // RTU: at least t3.5 of silence ends the frame, even at low baud rates.
        // ASCII: wait for CRLF; the outer response timeout bounds the wait.
        let quiet = match self.mode {
            SerialMode::Rtu => self.config.frame_gap.max(Duration::from_millis(5)),
            SerialMode::Ascii => Duration::from_millis(5),
        };
        self.read_frame_raw(quiet).await
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rtu_response_can_start_after_initial_silence() {
        let (mut tx, mut rx) = tokio::io::duplex(64);
        let writer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            tx.write_all(&[1, 3, 2, 0, 42]).await.unwrap();
            // Keep the simulated serial line open so t3.5 silence, not EOF, terminates the frame.
            tokio::time::sleep(Duration::from_millis(30)).await;
        });
        let mut buf = Vec::new();
        let frame = tokio::time::timeout(
            Duration::from_millis(300),
            read_frame_from(&mut rx, &mut buf, SerialMode::Rtu, Duration::from_millis(5)),
        ).await.unwrap().unwrap();
        assert_eq!(frame, vec![1, 3, 2, 0, 42]);
        writer.await.unwrap();
    }

    #[tokio::test]
    async fn ascii_waits_for_crlf_even_after_short_silence() {
        let (mut tx, mut rx) = tokio::io::duplex(64);
        let writer = tokio::spawn(async move {
            tx.write_all(b":01030200").await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            tx.write_all(b"2AF0\r\n").await.unwrap();
        });
        let mut buf = Vec::new();
        let frame = tokio::time::timeout(
            Duration::from_millis(300),
            read_frame_from(&mut rx, &mut buf, SerialMode::Ascii, Duration::from_millis(5)),
        ).await.unwrap().unwrap();
        assert_eq!(frame, b":010302002AF0\r\n");
        writer.await.unwrap();
    }

    #[test]
    fn rtu_gap_is_based_on_actual_character_duration() {
        assert_eq!(
            required_rtu_gap(1_200, DataBitsSerde::Eight, ParitySerde::None, StopBitsSerde::One),
            Duration::from_micros(29_167)
        );
        assert_eq!(
            required_rtu_gap(9_600, DataBitsSerde::Eight, ParitySerde::Even, StopBitsSerde::One),
            Duration::from_micros(4_011)
        );
        assert_eq!(
            required_rtu_gap(38_400, DataBitsSerde::Eight, ParitySerde::None, StopBitsSerde::One),
            Duration::from_micros(1_750)
        );
    }
}

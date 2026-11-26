//! Serial port commands: enumeration, quick open test.
//!
//! Notes:
//! - `list_serial_ports` uses `tokio_serial::available_ports` (SERIAL/D-Bus
//!   dependent on the platform, USB PID/VID are reported where available);
//! - `test_serial_port` opens the port with the given parameters, and if
//!   `test_modbus = true`, sends a probe request (FC 0x03, unit 1) and
//!   measures the response time.

use ts_rs::TS;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::modbus::common::SerialMode;
use crate::modbus::common::UnitId;
use crate::modbus::frame::request;
use crate::state::AppState;
use crate::transport::serial::{SerialPortConfig, SerialTransport};
use crate::transport::Transport;

/// Information about one available serial port.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SerialPortInfo {
    pub port_name: String,
    /// "usb", "pci", "bluetooth", "unknown"...
    pub port_type: String,
    /// USB info, if available.
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    /// Manufacturer / product / serial number (USB, if available).
    pub manufacturer: Option<String>,
    pub product: Option<String>,
}

/// Parameters for `test_serial_port`.
#[derive(Debug, Clone, Deserialize, TS)]
pub struct TestSerialParams {
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub parity: String,
    pub stop_bits: u8,
    /// RTU or ASCII.
    pub mode: SerialMode,
    /// Send a Modbus probe (FC 0x03 unit 1) after opening.
    #[serde(default = "default_true")]
    pub test_modbus: bool,
}

fn default_true() -> bool {
    true
}

/// Result of `test_serial_port`.
#[derive(Debug, Clone, Serialize, TS)]
pub struct TestSerialResult {
    pub opened: bool,
    pub error: Option<String>,
    /// Modbus probe outcome (if requested).
    pub modbus_responded: bool,
    /// Response time in microseconds (probe roundtrip).
    #[ts(type = "number | null")]
    pub rtt_us: Option<u128>,
    /// Raw response hex (if any).
    pub response_hex: Option<String>,
}

#[tauri::command]
pub async fn list_serial_ports() -> Result<Vec<SerialPortInfo>, String> {
    let ports = tokio_serial::available_ports().map_err(|e| e.to_string())?;
    Ok(ports
        .into_iter()
        .map(|p| {
            let (port_type, vid, pid, manufacturer, product) = match &p.port_type {
                tokio_serial::SerialPortType::UsbPort(info) => (
                    "usb".to_string(),
                    Some(info.vid),
                    Some(info.pid),
                    info.manufacturer.clone(),
                    info.product.clone(),
                ),
                tokio_serial::SerialPortType::PciPort => ("pci".to_string(), None, None, None, None),
                tokio_serial::SerialPortType::BluetoothPort => ("bluetooth".to_string(), None, None, None, None),
                _ => ("unknown".to_string(), None, None, None, None),
            };
            SerialPortInfo {
                port_name: p.port_name,
                port_type,
                vid,
                pid,
                manufacturer,
                product,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn test_serial_port(state: State<'_, AppState>, params: TestSerialParams) -> Result<TestSerialResult, String> {
    use crate::transport::serial::{DataBitsSerde, ParitySerde, StopBitsSerde};

    let data_bits = match params.data_bits {
        5 => DataBitsSerde::Five,
        6 => DataBitsSerde::Six,
        7 => DataBitsSerde::Seven,
        _ => DataBitsSerde::Eight,
    };
    let parity = match params.parity.as_str() {
        "even" => ParitySerde::Even,
        "odd" => ParitySerde::Odd,
        _ => ParitySerde::None,
    };
    let stop_bits = match params.stop_bits {
        2 => StopBitsSerde::Two,
        _ => StopBitsSerde::One,
    };

    let cfg = SerialPortConfig {
        port_name: params.port_name.clone(),
        baud_rate: params.baud_rate,
        data_bits,
        parity,
        stop_bits,
        frame_gap: Duration::from_millis(2),
    };

    let mut result = TestSerialResult {
        opened: false,
        error: None,
        modbus_responded: false,
        rtt_us: None,
        response_hex: None,
    };

    let communication=state.scanner.communication();
    let _lease=match communication.manager.lease_serial(&params.port_name){
        Ok(lease)=>lease,
        Err(e)=>{result.error=Some(e.to_string());return Ok(result);}
    };
    let mut transport = match SerialTransport::open(cfg, params.mode).await {
        Ok(t) => t,
        Err(e) => {
            result.error = Some(e.to_string());
            return Ok(result);
        }
    };
    result.opened = true;

    if params.test_modbus {
        // Build a read-holding-registers probe (unit 1, addr 0, qty 1).
        let probe = match params.mode {
            SerialMode::Rtu => crate::modbus::rtu::encode(UnitId(1), &request::read_holding_registers(0, 1)),
            SerialMode::Ascii => crate::modbus::ascii::encode(UnitId(1), &request::read_holding_registers(0, 1)),
        };
        let started = Instant::now();
        match transport.transact(&probe, Duration::from_millis(800), 7).await {
            Ok(resp) => {
                result.response_hex = Some(hex::encode(&resp));
                let valid = match params.mode {
                    SerialMode::Rtu => crate::modbus::rtu::decode(&resp)
                        .map(|frame| frame.unit_id.0 == 1 && (frame.pdu.function() == 0x03)),
                    SerialMode::Ascii => crate::modbus::ascii::decode(&resp)
                        .map(|frame| frame.unit_id.0 == 1 && (frame.pdu.function() == 0x03)),
                };
                match valid {
                    Ok(true) => {
                        result.modbus_responded = true;
                        result.rtt_us = Some(started.elapsed().as_micros());
                    }
                    Ok(false) => result.error = Some("unexpected Modbus response (unit/function)".into()),
                    Err(e) => result.error = Some(format!("invalid Modbus frame: {e}")),
                }
            }
            Err(e) => result.error = Some(e.to_string()),
        }
    }

    let _ = transport.close().await;
    Ok(result)
}

/// Prevent unused import warning (AppState used in other commands' signatures).
#[allow(dead_code)]
fn _state_used(_s: &State<AppState>) {}

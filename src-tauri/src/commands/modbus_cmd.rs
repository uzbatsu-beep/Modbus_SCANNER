//! Manual (non-scan) Modbus commands: read/write registers, raw frame send.
//!
//! These are for the "Manual / Debug" tab of the UI: one-shot requests
//! against a single device with full control over the PDU.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::modbus::common::{ModbusError, SerialMode, TransportKind, UnitId};
use crate::modbus::decode::{
    decode, interpret_all, DecodeResult, DecodeSpec, DecodedValue,
};
use crate::modbus::frame::{request, Pdu};
use crate::transport::serial::{SerialPortConfig, SerialTransport};
use crate::transport::tcp::TcpTransport;
use crate::transport::udp::UdpTransport;
use crate::transport::{Transport, TransportError};

use crate::state::AppState;

/// Manual command target (one device).
#[derive(Debug, Clone, Deserialize)]
pub struct ManualTarget {
    pub kind: TransportKind,
    /// Serial: port name; TCP/UDP: host.
    pub endpoint: String,
    pub port: Option<u16>,
    pub unit_id: u8,
    /// Serial params (required for RTU/ASCII).
    pub serial: Option<crate::scanner::types::SerialPortParams>,
    pub serial_mode: Option<SerialMode>,
}

/// Manual command timing.
#[derive(Debug, Clone, Deserialize)]
pub struct ManualTiming {
    pub timeout_ms: u32,
    #[serde(default = "default_retries")]
    pub retries: u32,
}

fn default_retries() -> u32 {
    0
}
/// Read request: one function code, address, quantity.
#[derive(Debug, Clone, Deserialize)]
pub struct ReadRequest {
    pub target: ManualTarget,
    /// Function code: 0x01..=0x04.
    pub function: u8,
    pub start_address: u16,
    pub quantity: u16,
    pub timing: Option<ManualTiming>,
}

/// Write request.
#[derive(Debug, Clone, Deserialize)]
pub struct WriteRequest {
    pub target: ManualTarget,
    pub function: u8,
    pub start_address: u16,
    /// Values to write (registers) or one value (coil: 0/1).
    pub values: Vec<u16>,
    pub timing: Option<ManualTiming>,
}

/// Raw frame request: full ADU bytes (hex) sent as is.
#[derive(Debug, Clone, Deserialize)]
pub struct RawFrameRequest {
    pub target: ManualTarget,
    /// Hex string of the raw ADU (without CRC for RTU — computed automatically;
    /// with MBAP for TCP).
    pub adu_hex: String,
    /// If false, `adu_hex` is a PDU and the ADU is built automatically.
    pub is_full_adu: bool,
    pub timing: Option<ManualTiming>,
}

/// Manual command response.
#[derive(Debug, Clone, Serialize)]
pub struct ManualResponse {
    pub ok: bool,
    /// Error message (if !ok).
    pub error: Option<String>,
    /// Modbus exception code (if the device answered with an exception).
    pub exception_code: Option<u8>,
    /// Parsed values (reads).
    pub values: Vec<u16>,
    /// Raw response ADU hex.
    pub response_hex: Option<String>,
    /// Roundtrip in microseconds.
    pub rtt_us: Option<u128>,
}

fn build_adu(kind: TransportKind, unit_id: u8, pdu: &[u8]) -> Vec<u8> {
    match kind {
        TransportKind::Rtu => crate::modbus::rtu::encode(UnitId(unit_id), pdu),
        TransportKind::Ascii => crate::modbus::ascii::encode(UnitId(unit_id), pdu),
        TransportKind::Tcp => {
            let mut frame = Vec::with_capacity(7 + pdu.len());
            frame.extend_from_slice(&0u16.to_be_bytes());
            frame.extend_from_slice(&0u16.to_be_bytes());
            frame.extend_from_slice(&((1 + pdu.len()) as u16).to_be_bytes());
            frame.push(unit_id);
            frame.extend_from_slice(pdu);
            frame
        }
        TransportKind::Udp => crate::modbus::rtu::encode(UnitId(unit_id), pdu),
    }
}

async fn manual_transact(
    req: &impl HasTarget,
    adu: Vec<u8>,
    expected_hint: usize,
) -> Result<(Vec<u8>, u128), TransportError> {
    let target = req.target();
    let timing = req.timing();
    let timeout = Duration::from_millis(timing.map(|t| t.timeout_ms as u64).unwrap_or(800));
    let retries = timing.map(|t| t.retries).unwrap_or(0);

    let mut attempts = 0u32;
    loop {
        let started = Instant::now();
        let res: Result<Vec<u8>, TransportError> = match target.kind {
            TransportKind::Rtu | TransportKind::Ascii => {
                let mode = match target.serial_mode {
                    Some(SerialMode::Ascii) => SerialMode::Ascii,
                    _ => SerialMode::Rtu,
                };
                let sp = target
                    .serial
                    .clone()
                    .ok_or_else(|| TransportError::Other("serial params required".into()))?;
                let cfg = SerialPortConfig {
                    port_name: sp.port_name,
                    baud_rate: sp.baud_rate,
                    data_bits: sp.data_bits.into(),
                    parity: sp.parity.into(),
                    stop_bits: sp.stop_bits.into(),
                    frame_gap: Duration::from_millis(2),
                };
                let mut t = SerialTransport::open(cfg, mode).await?;
                let res = t.transact(&adu, timeout, expected_hint).await;
                let _ = t.close().await;
                res
            }
            TransportKind::Tcp => {
                let mut t = TcpTransport::connect(crate::transport::tcp::TcpConfig {
                    host: target.endpoint.clone(),
                    port: target.port.unwrap_or(502),
                    connect_timeout: timeout,
                    framing: crate::transport::tcp::TcpFraming::Mbap,
                tls: None,
                })
                .await?;
                let res = t.transact(&adu, timeout, expected_hint).await;
                let _ = t.close().await;
                res
            }
            TransportKind::Udp => {
                let mut t = UdpTransport::connect(&target.endpoint, target.port.unwrap_or(502)).await?;
                t.transact(&adu, timeout, expected_hint).await
            }
        };

        match res {
            Ok(resp) => return Ok((resp, started.elapsed().as_micros())),
            Err(e) => {
                attempts += 1;
                if attempts > retries {
                    return Err(e);
                }
            }
        }
    }
}

trait HasTarget {
    fn target(&self) -> &ManualTarget;
    fn timing(&self) -> Option<&ManualTiming>;
}

impl HasTarget for ReadRequest {
    fn target(&self) -> &ManualTarget {
        &self.target
    }
    fn timing(&self) -> Option<&ManualTiming> {
        self.timing.as_ref()
    }
}

impl HasTarget for WriteRequest {
    fn target(&self) -> &ManualTarget {
        &self.target
    }
    fn timing(&self) -> Option<&ManualTiming> {
        self.timing.as_ref()
    }
}

impl HasTarget for RawFrameRequest {
    fn target(&self) -> &ManualTarget {
        &self.target
    }
    fn timing(&self) -> Option<&ManualTiming> {
        self.timing.as_ref()
    }
}

fn verify_and_parse(
    kind: TransportKind,
    unit_id: u8,
    expected_fc: u8,
    resp: &[u8],
) -> Result<Pdu, ModbusError> {
    let pdu = match kind {
        TransportKind::Rtu => {
            let f = crate::modbus::rtu::decode(resp)?;
            if f.unit_id.0 != unit_id {
                return Err(ModbusError::UnitIdMismatch {
                    expected: unit_id,
                    actual: f.unit_id.0,
                });
            }
            f.pdu
        }
        TransportKind::Ascii => {
            let f = crate::modbus::ascii::decode(resp)?;
            if f.unit_id.0 != unit_id {
                return Err(ModbusError::UnitIdMismatch {
                    expected: unit_id,
                    actual: f.unit_id.0,
                });
            }
            f.pdu
        }
        TransportKind::Tcp => {
            if resp.len() < 8 {
                return Err(ModbusError::TooShort(resp.len()));
            }
            if resp[6] != unit_id {
                return Err(ModbusError::UnitIdMismatch {
                    expected: unit_id,
                    actual: resp[6],
                });
            }
            Pdu::from_bytes(&resp[7..])?
        }
        TransportKind::Udp => {
            let f = crate::modbus::rtu::decode(resp)?;
            if f.unit_id.0 != unit_id {
                return Err(ModbusError::UnitIdMismatch {
                    expected: unit_id,
                    actual: f.unit_id.0,
                });
            }
            f.pdu
        }
    };

    if !pdu.is_exception() && pdu.function() != expected_fc {
        return Err(ModbusError::UnexpectedFunction(pdu.function()));
    }
    Ok(pdu)
}

/// All interpretations and float hints for 1–2 selected raw registers (P3-01/02).
#[tauri::command]
pub fn inspect_registers(values: Vec<u16>) -> DecodeResult {
    interpret_all(&values)
}

/// Decode with an explicit spec (data type, byte order, scaling) — used by named tags.
#[tauri::command]
pub fn decode_register(values: Vec<u16>, spec: DecodeSpec) -> Option<DecodedValue> {
    decode(&values, &spec)
}

#[tauri::command]
pub async fn read_registers(
    state: State<'_, AppState>,
    req: ReadRequest,
) -> Result<ManualResponse, String> {
    let _ = &state;
    let expected_fc = req.function;
    let pdu = match expected_fc {
        0x01 => request::read_coils(req.start_address, req.quantity),
        0x02 => request::read_discrete_inputs(req.start_address, req.quantity),
        0x03 => request::read_holding_registers(req.start_address, req.quantity),
        0x04 => request::read_input_registers(req.start_address, req.quantity),
        other => return Err(format!("unsupported read function {other:#04x}")),
    };
    let adu = build_adu(req.target.kind, req.target.unit_id, &pdu);

    match manual_transact(&req, adu, 3 + (req.quantity as usize) * 2).await {
        Ok((resp, rtt)) => match verify_and_parse(req.target.kind, req.target.unit_id, expected_fc, &resp) {
            Ok(p) if p.is_exception() => {
                if let Pdu::Exception { code, .. } = p {
                    Ok(ManualResponse {
                        ok: false,
                        error: Some(ModbusError::from_exception(code).to_string()),
                        exception_code: Some(code),
                        values: Vec::new(),
                        response_hex: Some(hex::encode(&resp)),
                        rtt_us: Some(rtt),
                    })
                } else {
                    unreachable!()
                }
            }
            Ok(p) => {
                let data = match &p {
                    Pdu::Response { data, .. } => data.clone(),
                    _ => Vec::new(),
                };
                let values = match expected_fc {
                    0x01 | 0x02 => crate::modbus::frame::response::parse_bits(&data, req.quantity)
                        .map(|bits| bits.iter().map(|&b| b as u16).collect())
                        .unwrap_or_default(),
                    _ => crate::modbus::frame::response::parse_registers(&data).unwrap_or_default(),
                };
                Ok(ManualResponse {
                    ok: true,
                    error: None,
                    exception_code: None,
                    values,
                    response_hex: Some(hex::encode(&resp)),
                    rtt_us: Some(rtt),
                })
            }
            Err(e) => Ok(ManualResponse {
                ok: false,
                error: Some(e.to_string()),
                exception_code: None,
                values: Vec::new(),
                response_hex: Some(hex::encode(&resp)),
                rtt_us: Some(rtt),
            }),
        },
        Err(e) => Ok(ManualResponse {
            ok: false,
            error: Some(e.to_string()),
            exception_code: None,
            values: Vec::new(),
            response_hex: None,
            rtt_us: None,
        }),
    }
}

#[tauri::command]
pub async fn write_registers(
    state: State<'_, AppState>,
    req: WriteRequest,
) -> Result<ManualResponse, String> {
    let _ = &state;
    let expected_fc = req.function;
    let pdu = match expected_fc {
        0x05 => {
            if req.values.len() != 1 {
                return Err("FC 0x05 requires exactly one value".into());
            }
            request::write_single_coil(req.start_address, req.values[0] != 0)
        }
        0x06 => {
            if req.values.len() != 1 {
                return Err("FC 0x06 requires exactly one value".into());
            }
            request::write_single_register(req.start_address, req.values[0])
        }
        0x10 => request::write_multiple_registers(req.start_address, &req.values),
        other => return Err(format!("unsupported write function {other:#04x}")),
    };
    let adu = build_adu(req.target.kind, req.target.unit_id, &pdu);

    match manual_transact(&req, adu, 8).await {
        Ok((resp, rtt)) => match verify_and_parse(req.target.kind, req.target.unit_id, expected_fc, &resp) {
            Ok(p) if p.is_exception() => {
                if let Pdu::Exception { code, .. } = p {
                    Ok(ManualResponse {
                        ok: false,
                        error: Some(ModbusError::from_exception(code).to_string()),
                        exception_code: Some(code),
                        values: Vec::new(),
                        response_hex: Some(hex::encode(&resp)),
                        rtt_us: Some(rtt),
                    })
                } else {
                    unreachable!()
                }
            }
            Ok(_) => Ok(ManualResponse {
                ok: true,
                error: None,
                exception_code: None,
                values: req.values.clone(),
                response_hex: Some(hex::encode(&resp)),
                rtt_us: Some(rtt),
            }),
            Err(e) => Ok(ManualResponse {
                ok: false,
                error: Some(e.to_string()),
                exception_code: None,
                values: Vec::new(),
                response_hex: Some(hex::encode(&resp)),
                rtt_us: Some(rtt),
            }),
        },
        Err(e) => Ok(ManualResponse {
            ok: false,
            error: Some(e.to_string()),
            exception_code: None,
            values: Vec::new(),
            response_hex: None,
            rtt_us: None,
        }),
    }
}

#[tauri::command]
pub async fn send_raw_frame(
    state: State<'_, AppState>,
    req: RawFrameRequest,
) -> Result<ManualResponse, String> {
    let _ = &state;
    let bytes = hex::decode(req.adu_hex.trim().replace(' ', ""))
        .map_err(|e| format!("bad hex: {e}"))?;

    let adu = if req.is_full_adu {
        bytes
    } else {
        build_adu(req.target.kind, req.target.unit_id, &bytes)
    };

    if adu.is_empty() {
        return Err("empty frame".into());
    }

    match manual_transact(&req, adu.clone(), 8).await {
        Ok((resp, rtt)) => Ok(ManualResponse {
            ok: true,
            error: None,
            exception_code: None,
            values: Vec::new(),
            response_hex: Some(hex::encode(&resp)),
            rtt_us: Some(rtt),
        }),
        Err(e) => Ok(ManualResponse {
            ok: false,
            error: Some(e.to_string()),
            exception_code: None,
            values: Vec::new(),
            response_hex: None,
            rtt_us: None,
        }),
    }
}

/// Suppress unused import warnings for types used only in signatures.
#[allow(dead_code)]
fn _suppress(_t: &TransportError, _u: &UnitId) {}

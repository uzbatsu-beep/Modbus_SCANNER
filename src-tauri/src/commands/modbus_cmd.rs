//! Manual (non-scan) Modbus commands: read/write registers, raw frame send.
//!
//! These are for the "Manual / Debug" tab of the UI: one-shot requests
//! against a single device with full control over the PDU.

use ts_rs::TS;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::modbus::common::{ModbusError, SerialMode, TransportKind, UnitId};
use crate::modbus::decode::{
    decode, interpret_all, DecodeResult, DecodeSpec, DecodedValue,
};
use crate::modbus::frame::{
    parse_tcp_response, request, response, validate_response_function, Pdu,
};
use crate::transport::serial::{SerialPortConfig, SerialTransport};
use crate::transport::tcp::TcpTransport;
use crate::transport::udp::UdpTransport;
use crate::transport::{Transport, TransportError};

use crate::state::AppState;

/// Manual command target (one device).
#[derive(Debug, Clone, Deserialize, TS)]
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
#[derive(Debug, Clone, Deserialize, TS)]
pub struct ManualTiming {
    pub timeout_ms: u32,
    #[serde(default = "default_retries")]
    pub retries: u32,
}

fn default_retries() -> u32 {
    0
}
/// Read request: one function code, address, quantity.
#[derive(Debug, Clone, Deserialize, TS)]
pub struct ReadRequest {
    pub target: ManualTarget,
    /// Function code: 0x01..=0x04.
    pub function: u8,
    pub start_address: u16,
    pub quantity: u16,
    pub timing: Option<ManualTiming>,
}

/// Write request.
#[derive(Debug, Clone, Deserialize, TS)]
pub struct WriteRequest {
    pub target: ManualTarget,
    pub function: u8,
    pub start_address: u16,
    /// Values to write (registers) or one value (coil: 0/1).
    pub values: Vec<u16>,
    /// Opt-in for a single manual write (false for older clients).
    #[serde(default)]
    pub confirm_write: bool,
    pub timing: Option<ManualTiming>,
}

/// Raw frame request: full ADU bytes (hex) sent as is.
#[derive(Debug, Clone, Deserialize, TS)]
pub struct RawFrameRequest {
    pub target: ManualTarget,
    /// Hex string of the raw ADU (without CRC for RTU — computed automatically;
    /// with MBAP for TCP).
    pub adu_hex: String,
    /// If false, `adu_hex` is a PDU and the ADU is built automatically.
    pub is_full_adu: bool,
    /// An arbitrary raw frame requires an explicit per-request opt-in.
    #[serde(default)]
    pub confirm_unsafe: bool,
    pub timing: Option<ManualTiming>,
}

/// Manual command response.
#[derive(Debug, Clone, Serialize, TS)]
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
    #[ts(type = "number | null")]
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

/// Manual writes and arbitrary raw frames can change equipment state. A lost
/// response does NOT prove that the command was not executed, so never retry
/// these operations automatically, even when a caller configured read retries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManualRetryPolicy {
    ReadOnly,
    SingleShot,
}

fn permitted_retries(timing: Option<&ManualTiming>, policy: ManualRetryPolicy) -> u32 {
    match policy {
        ManualRetryPolicy::ReadOnly => timing.map(|t| t.retries.min(5)).unwrap_or(0),
        ManualRetryPolicy::SingleShot => 0,
    }
}

fn write_outcome_unknown(error: &TransportError) -> String {
    format!(
        "Write/raw outcome is not confirmed ({error}). The device may have executed the command.          Automatic retries are disabled; read back device state before retrying manually."
    )
}

/// Confirm a positive write acknowledgment refers to the *same* address and
/// value/quantity that were transmitted. Matching only FC and unit ID is unsafe.
fn verify_write_ack(response: &Pdu, request: &[u8]) -> Result<(), ModbusError> {
    let Some(&function) = request.first() else {
        return Err(ModbusError::InvalidFrame("empty write PDU".into()));
    };
    let Pdu::Response { function: reply_fc, data } = response else {
        return Err(ModbusError::InvalidFrame("expected positive write acknowledgment".into()));
    };
    if *reply_fc != function {
        return Err(ModbusError::UnexpectedFunction(*reply_fc));
    }
    let expected_data: &[u8] = match function {
        0x05 | 0x06 if request.len() == 5 => &request[1..],
        0x10 if request.len() >= 6 => &request[1..5],
        _ => return Err(ModbusError::InvalidFrame(format!(
            "unsupported or malformed write request FC {function:#04x}"
        ))),
    };
    if data.as_slice() != expected_data {
        return Err(ModbusError::InvalidFrame(format!(
            "FC {function:#04x} write acknowledgment does not echo the requested address/value/quantity"
        )));
    }
    Ok(())
}

async fn manual_transact(
    req: &impl HasTarget,
    adu: Vec<u8>,
    expected_hint: usize,
    policy: ManualRetryPolicy,
) -> Result<(Vec<u8>, u128), TransportError> {
    let target = req.target();
    let timing = req.timing();
    let timeout = Duration::from_millis(
        timing.map(|t| t.timeout_ms).unwrap_or(800).clamp(50, 10_000) as u64,
    );
    let retries = permitted_retries(timing, policy);

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
        // Manual requests use transaction id 0 in build_adu.
        TransportKind::Tcp => parse_tcp_response(resp, unit_id, 0)?,
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

    validate_response_function(pdu, expected_fc)
}

/// Reject broadcast destinations encoded directly inside a caller-supplied ADU.
/// Checking only target.unit_id is insufficient in full-ADU mode.
fn is_raw_broadcast(kind: TransportKind, payload: &[u8]) -> bool {
    match kind {
        TransportKind::Rtu | TransportKind::Udp => payload.first() == Some(&0),
        TransportKind::Tcp => payload.get(6) == Some(&0),
        TransportKind::Ascii => payload.len() >= 3 &&
            payload[0] == b':' && payload[1] == b'0' && payload[2] == b'0',
    }
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

/// Validate address and size before constructing any read PDU.
fn validate_read(function: u8, address: u16, quantity: u16) -> Result<(), String> {
    let max = match function {
        0x01 | 0x02 => 2_000,
        0x03 | 0x04 => 125,
        _ => return Err(format!("unsupported read function {function:#04x}")),
    };
    if quantity == 0 || quantity > max || address as u32 + quantity as u32 > 65_536 {
        return Err(format!("invalid read address/quantity ({address}, {quantity})"));
    }
    Ok(())
}

fn validate_write(function: u8, address: u16, values: &[u16]) -> Result<(), String> {
    let ok = match function {
        0x05 => values.len() == 1 && values[0] <= 1,
        0x06 => values.len() == 1,
        0x10 => (1..=123).contains(&values.len()),
        _ => return Err(format!("unsupported write function {function:#04x}")),
    };
    if !ok || address as u32 + values.len() as u32 > 65_536 {
        return Err(format!("invalid write address/value count ({address}, {})", values.len()));
    }
    Ok(())
}

#[tauri::command]
pub async fn read_registers(
    state: State<'_, AppState>,
    req: ReadRequest,
) -> Result<ManualResponse, String> {
    let _ = &state;
    validate_read(req.function, req.start_address, req.quantity)?;
    let expected_fc = req.function;
    let pdu = match expected_fc {
        0x01 => request::read_coils(req.start_address, req.quantity),
        0x02 => request::read_discrete_inputs(req.start_address, req.quantity),
        0x03 => request::read_holding_registers(req.start_address, req.quantity),
        0x04 => request::read_input_registers(req.start_address, req.quantity),
        other => return Err(format!("unsupported read function {other:#04x}")),
    };
    let adu = build_adu(req.target.kind, req.target.unit_id, &pdu);

    match manual_transact(&req, adu, 3 + (req.quantity as usize) * 2, ManualRetryPolicy::ReadOnly).await {
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
                let parsed = match &p {
                    Pdu::Response { data, .. } => {
                        response::parse_read_values(expected_fc, data, req.quantity)
                    }
                    _ => Err(ModbusError::InvalidFrame(
                        "manual read expected a normal response PDU".into()
                    )),
                };
                match parsed {
                    Ok(values) => Ok(ManualResponse {
                        ok: true,
                        error: None,
                        exception_code: None,
                        values,
                        response_hex: Some(hex::encode(&resp)),
                        rtt_us: Some(rtt),
                    }),
                    Err(error) => Ok(ManualResponse {
                        ok: false,
                        error: Some(error.to_string()),
                        exception_code: None,
                        values: Vec::new(),
                        response_hex: Some(hex::encode(&resp)),
                        rtt_us: Some(rtt),
                    }),
                }
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
    if !req.confirm_write { return Err("explicit write confirmation required".into()); }
    if req.target.unit_id == 0 {
        return Err("broadcast writes (unit 0) are blocked on every transport".into());
    }
    validate_write(req.function, req.start_address, &req.values)?;
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

    match manual_transact(&req, adu, 8, ManualRetryPolicy::SingleShot).await {
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
            Ok(p) => match verify_write_ack(&p, &pdu) {
                Ok(()) => Ok(ManualResponse {
                    ok: true,
                    error: None,
                    exception_code: None,
                    values: req.values.clone(),
                    response_hex: Some(hex::encode(&resp)),
                    rtt_us: Some(rtt),
                }),
                Err(error) => Ok(ManualResponse {
                    ok: false,
                    error: Some(error.to_string()),
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
                response_hex: Some(hex::encode(&resp)),
                rtt_us: Some(rtt),
            }),
        },
        Err(e) => Ok(ManualResponse {
            ok: false,
            error: Some(write_outcome_unknown(&e)),
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
    if !req.confirm_unsafe { return Err("explicit unsafe-frame confirmation required".into()); }
    if req.target.unit_id == 0 {
        return Err("broadcast raw requests (unit 0) are blocked on every transport".into());
    }
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
    if req.is_full_adu && is_raw_broadcast(req.target.kind, &adu) {
        return Err("broadcast raw ADU (unit 0) is blocked".into());
    }

    match manual_transact(&req, adu, 8, ManualRetryPolicy::SingleShot).await {
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
            error: Some(write_outcome_unknown(&e)),
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


#[cfg(test)]
mod reliability_tests {
    use super::*;

    #[test]
    fn manual_tcp_response_exposes_register_data() {
        // Tx=0, protocol=0, length=5 (UnitId+FC+byteCount+2 data bytes).
        let adu = [0, 0, 0, 0, 0, 5, 1, 3, 2, 0x12, 0x34];
        let response = verify_and_parse(TransportKind::Tcp, 1, 0x03, &adu).unwrap();
        match response {
            Pdu::Response { function, data } => {
                assert_eq!(function, 0x03);
                assert_eq!(crate::modbus::frame::response::parse_registers(&data).unwrap(), [0x1234]);
            }
            other => panic!("expected typed response, got {other:?}"),
        }
    }

    #[test]
    fn manual_tcp_rejects_wrong_tid_protocol_length_and_fc() {
        let good = [0, 0, 0, 0, 0, 5, 1, 3, 2, 0x12, 0x34];
        for (position, value) in [(0, 1), (3, 1), (5, 4), (6, 2), (7, 4)] {
            let mut broken = good;
            broken[position] = value;
            assert!(
                verify_and_parse(TransportKind::Tcp, 1, 3, &broken).is_err(),
                "manual TCP accepted malformed MBAP/PDU at offset {position}"
            );
        }
        let short_register_data = [0, 0, 0, 0, 0, 4, 1, 3, 2, 0x12];
        let p = verify_and_parse(TransportKind::Tcp, 1, 3, &short_register_data)
            .expect("MBAP alone cannot determine the requested register count");
        match p {
            Pdu::Response { data, .. } => assert!(
                response::parse_read_values(3, &data, 1).is_err()
            ),
            _ => panic!("expected raw read response"),
        }
        let wrong_exception = [0, 0, 0, 0, 0, 3, 1, 0x84, 2];
        assert!(verify_and_parse(TransportKind::Tcp, 1, 3, &wrong_exception).is_err());
    }

    #[test]
    fn manual_bit_reads_do_not_convert_bad_count_into_success() {
        assert_eq!(response::parse_read_values(1, &[1, 0x01], 1).unwrap(), [1]);
        assert!(response::parse_read_values(1, &[1, 0x01], 16).is_err());
        assert!(response::parse_read_values(2, &[1, 0x01, 0x02], 1).is_err());
    }

    #[test]
    fn full_raw_adu_cannot_bypass_broadcast_protection() {
        assert!(is_raw_broadcast(TransportKind::Rtu, &[0, 6, 0, 0]));
        assert!(is_raw_broadcast(TransportKind::Ascii, b":00060000"));
        assert!(is_raw_broadcast(TransportKind::Tcp, &[0, 1, 0, 0, 0, 6, 0, 6]));
        assert!(!is_raw_broadcast(TransportKind::Tcp, &[0, 1, 0, 0, 0, 6, 1, 6]));
    }

    #[test]
    fn validates_function_quantity_and_address_bounds() {
        assert!(validate_read(0x03, 0, 125).is_ok());
        assert!(validate_read(0x03, 0, 126).is_err());
        assert!(validate_read(0x03, 0, 0).is_err());
        assert!(validate_read(0x01, u16::MAX, 2).is_err());
        assert!(validate_read(0x10, 0, 1).is_err());
        assert!(validate_write(0x10, 0, &vec![0; 123]).is_ok());
        assert!(validate_write(0x10, 0, &vec![0; 124]).is_err());
        assert!(validate_write(0x10, 0, &[]).is_err());
        assert!(validate_write(0x05, 0, &[2]).is_err());
        assert!(validate_write(0x10, u16::MAX, &[1, 2]).is_err());
    }
}

//! Serial port commands: enumeration, quick open test.
//!
//! Notes:
//! - `list_serial_ports` uses `tokio_serial::available_ports` (SERIAL/D-Bus
//!   dependent on the platform, USB PID/VID are reported where available);
//! - `test_serial_port` opens the port with the given parameters, and if
//!   `test_modbus = true`, sends a probe request (FC 0x03, unit 1) and
//!   measures the response time using a managed connection. Probe fields are configurable.

use ts_rs::TS;
use std::time::Duration;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::modbus::common::SerialMode;
use crate::communication::{configuration::serial_definition,service::{CommunicationService,ProtocolRequest,framing}};
use crate::modbus::frame::{request,Pdu};
use crate::scanner::{engine::verify_adu,types::SerialPortParams};
use crate::transport::connection::{ConnectionManager,Priority};
use crate::state::AppState;

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
    /// False: open/close only; true: one read-only Modbus request.
    #[serde(default = "default_true")]
    pub test_modbus: bool,
    pub unit_id: Option<u8>,
    pub function: Option<u8>,
    pub address: Option<u16>,
    pub quantity: Option<u16>,
    pub timeout_ms: Option<u32>,
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
    /// Valid exception confirms communication, not successful reading.
    pub exception_code: Option<u8>,
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
    run_serial_test(state.scanner.communication().manager.clone(),params).await
}

async fn run_serial_test(manager:Arc<ConnectionManager>,params:TestSerialParams)->Result<TestSerialResult,String>{
    use crate::transport::serial::{DataBitsSerde,ParitySerde,StopBitsSerde};
    let data_bits=match params.data_bits{5=>DataBitsSerde::Five,6=>DataBitsSerde::Six,7=>DataBitsSerde::Seven,8=>DataBitsSerde::Eight,_=>return Err("invalid data bits".into())};
    let parity=match params.parity.as_str(){"even"=>ParitySerde::Even,"odd"=>ParitySerde::Odd,"none"=>ParitySerde::None,_=>return Err("invalid parity".into())};
    let stop_bits=match params.stop_bits{1=>StopBitsSerde::One,2=>StopBitsSerde::Two,_=>return Err("invalid stop bits".into())};
    let definition=serial_definition("Serial diagnostic",&SerialPortParams{
        port_name:params.port_name.clone(),baud_rate:params.baud_rate,data_bits,parity,stop_bits,
    },params.mode);
    definition.resource_key().map_err(|e|e.to_string())?;
    let mut result=TestSerialResult{opened:false,error:None,modbus_responded:false,exception_code:None,rtt_us:None,response_hex:None};
    if !params.test_modbus{
        let mut session=match manager.exclusive_serial(&params.port_name,"Port Test").await{
            Ok(s)=>s,Err(e)=>{result.error=Some(e.to_string());return Ok(result);}
        };
        match session.test_open(definition).await{
            Ok(())=>result.opened=true,Err(e)=>result.error=Some(e.to_string()),
        }
        if let Err(e)=session.close().await{result.error=Some(e.to_string());}
        return Ok(result);
    }
    let unit=params.unit_id.unwrap_or(1);
    let fc=params.function.unwrap_or(3);
    let address=params.address.unwrap_or(0);
    let quantity=params.quantity.unwrap_or(1);
    let timeout=params.timeout_ms.unwrap_or(800);
    let max=match fc{1|2=>2000,3|4=>125,_=>return Err("diagnostic function must be FC01–FC04".into())};
    if !(1..=247).contains(&unit)||quantity==0||quantity>max||u32::from(address)+u32::from(quantity)>65536||!(50..=10_000).contains(&timeout){
        return Err("invalid diagnostic unit, span or timeout (50–10000 ms)".into());
    }
    let pdu=match fc{1=>request::read_coils(address,quantity),2=>request::read_discrete_inputs(address,quantity),
        3=>request::read_holding_registers(address,quantity),_=>request::read_input_registers(address,quantity)};
    let lease=match manager.acquire(definition){Ok(l)=>l,Err(e)=>{result.error=Some(e.to_string());return Ok(result);}};
    let (kind,engine)=framing(lease.definition());
    let outcome=CommunicationService::execute_leased(&lease,ProtocolRequest{
        unit,pdu,timeout:Duration::from_millis(u64::from(timeout)),priority:Priority::High,
    }).await;
    match outcome{
        Err(e)=>result.error=Some(e.to_string()),
        Ok(protocol)=>{
            let tx=protocol.transaction_id;
            let o=protocol.transaction;
            result.opened=o.timing.failure_stage.as_deref()!=Some("connect") && (o.timing.connect_us.is_some()||o.timing.reset_us.is_some()||o.timing.tx_us.is_some());
            if let Some(bytes)=o.rejected_response{result.response_hex=Some(hex::encode(bytes));}
            match o.result{
                Ok(bytes)=>{
                    result.opened=true;result.response_hex=Some(hex::encode(&bytes));
                    match verify_adu(kind,unit,fc,&bytes,tx,engine){
                        Ok(decoded)=>{
                            result.modbus_responded=true;
                            result.rtt_us=o.timing.rtt_us.map(u128::from);
                            if let Pdu::Exception{code,..}=decoded{result.exception_code=Some(code);}
                        },
                        Err(e)=>result.error=Some(e.to_string()),
                    }
                },
                Err(e)=>result.error=Some(e.to_string()),
            }
        }
    }
    lease.stop().await;
    Ok(result)
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::transport::{Transport,TransportResult,mock::MockTransport};
    use crate::transport::connection::{TransportFactory,TransportConfig};
    use crate::modbus::common::UnitId;
    use std::sync::atomic::{AtomicUsize,Ordering};
    struct Factory{opens:AtomicUsize,mock:MockTransport}
    #[async_trait::async_trait]
    impl TransportFactory for Factory{
        async fn open(&self,_:&TransportConfig)->TransportResult<Box<dyn Transport>>{
            self.opens.fetch_add(1,Ordering::SeqCst);Ok(Box::new(self.mock.clone()))
        }
    }
    fn params(test_modbus:bool)->TestSerialParams{
        TestSerialParams{port_name:if cfg!(windows){"COM99"}else{"/dev/modbus-diagnostic"}.into(),baud_rate:9600,data_bits:8,
            parity:"none".into(),stop_bits:1,mode:SerialMode::Rtu,test_modbus,
            unit_id:Some(7),function:Some(4),address:Some(12),quantity:Some(2),timeout_ms:Some(100)}
    }
    #[tokio::test]
    async fn port_test_opens_closes_without_tx(){
        let factory=Arc::new(Factory{opens:AtomicUsize::new(0),mock:MockTransport::new(vec![])});
        let manager=Arc::new(ConnectionManager::with_factory(factory.clone()));
        let r=run_serial_test(manager.clone(),params(false)).await.unwrap();
        assert!(r.opened);assert!(!r.modbus_responded);
        assert_eq!(factory.opens.load(Ordering::SeqCst),1);
        assert!(factory.mock.sent.lock().unwrap().is_empty());
        assert!(factory.mock.closed.load(Ordering::SeqCst));assert!(manager.snapshot().is_empty());
    }
    #[tokio::test]
    async fn diagnostic_uses_selected_read_and_valid_exception_confirms_link(){
        let response=crate::modbus::rtu::encode(UnitId(7),&[0x84,2]);
        let factory=Arc::new(Factory{opens:AtomicUsize::new(0),mock:MockTransport::new(vec![Ok(response)])});
        let manager=Arc::new(ConnectionManager::with_factory(factory.clone()));
        let r=run_serial_test(manager.clone(),params(true)).await.unwrap();
        assert!(r.opened&&r.modbus_responded);assert_eq!(r.exception_code,Some(2));assert!(r.error.is_none()&&r.rtt_us.is_some());
        assert_eq!(factory.mock.sent.lock().unwrap().as_slice(),&[crate::modbus::rtu::encode(UnitId(7),&[4,0,12,0,2])]);
        manager.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn invalid_diagnostic_function_and_span_rejected_before_open(){
        let factory=Arc::new(Factory{opens:AtomicUsize::new(0),mock:MockTransport::new(vec![])});
        let manager=Arc::new(ConnectionManager::with_factory(factory.clone()));
        let mut p=params(true);p.function=Some(6);
        assert!(run_serial_test(manager.clone(),p).await.is_err());
        let mut p=params(true);p.address=Some(65535);
        assert!(run_serial_test(manager.clone(),p).await.is_err());
        assert_eq!(factory.opens.load(Ordering::SeqCst),0);assert!(manager.snapshot().is_empty());
    }
    #[tokio::test]
    async fn wrong_quantity_crc_and_lrc_do_not_confirm_link(){
        for (mode,bad) in [
            (SerialMode::Rtu,crate::modbus::rtu::encode(UnitId(7),&[4,2,0,42])),
            (SerialMode::Rtu,vec![7,4,4,0,1,0,2,0,0]),
            (SerialMode::Ascii,b":0704040001000200\r\n".to_vec()),
        ]{
            let factory=Arc::new(Factory{opens:AtomicUsize::new(0),mock:MockTransport::new(vec![Ok(bad)])});
            let manager=Arc::new(ConnectionManager::with_factory(factory));
            let mut p=params(true);p.mode=mode;
            let r=run_serial_test(manager.clone(),p).await.unwrap();
            assert!(r.opened&&!r.modbus_responded);assert!(r.error.is_some()&&r.response_hex.is_some());
            manager.shutdown().await.unwrap();
        }
    }
}

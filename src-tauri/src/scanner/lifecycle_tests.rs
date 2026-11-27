//! Lifecycle regression tests using an isolated TCP loopback listener.
//! Assert the absence of new outbound requests after Stop, not just UI status.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::engine::{ControlMsg, ScannerHandle};
use super::types::{DeviceTarget, RegisterRange, ScanParams, ScanStatus, ScanTarget};
use super::EventSink;
use crate::modbus::common::TransportKind;

fn target(port: u16, register_plan: Vec<RegisterRange>) -> ScanTarget {
    ScanTarget::SingleDevice {
        target: DeviceTarget {
            kind: TransportKind::Tcp,
            endpoint: "127.0.0.1".into(),
            port: Some(port),
            unit_id: 1,
            serial: None,
            serial_mode: None,
        },
        register_plan,
    }
}

fn params(functions: Vec<u8>) -> ScanParams {
    ScanParams {
        probe_functions: functions,
        timeout_ms: 1_000,
        retries: 0,
        ip_concurrency: 1,
        request_gap_ms: 400,
        identify_devices: false,
        ip_allowlist: vec!["127.0.0.1/32".into()],
        ..ScanParams::default()
    }
}

async fn wait_status(scanner: &ScannerHandle, desired: ScanStatus) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while scanner.status() != desired {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("scanner never entered expected status");
}

async fn recv_request(listener: &TcpListener) -> (tokio::net::TcpStream, Vec<u8>) {
    let (mut stream, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
        .await.expect("expected TCP connection").expect("accept loopback");
    let mut header = [0u8; 7];
    stream.read_exact(&mut header).await.expect("read MBAP");
    let remaining = u16::from_be_bytes([header[4], header[5]]) as usize - 1;
    let mut body = vec![0u8; remaining];
    stream.read_exact(&mut body).await.expect("read request PDU");
    let mut frame = header.to_vec();
    frame.extend_from_slice(&body);
    (stream, frame)
}

async fn send_read_response(stream: &mut tokio::net::TcpStream, request: &[u8]) {
    let mut reply = request[..7].to_vec();
    reply[4] = 0;
    reply[5] = 5; // Unit ID + FC + byte count + register bytes
    reply.extend_from_slice(&[request[7], 2, 0, 42]);
    stream.write_all(&reply).await.expect("send Modbus TCP response");
}

async fn assert_no_more_connections(listener: &TcpListener) {
    assert!(
        tokio::time::timeout(Duration::from_millis(650), listener.accept()).await.is_err(),
        "new TCP request was made after Stop"
    );
}

#[tokio::test]
async fn second_start_is_rejected_during_running_and_paused() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let scanner = ScannerHandle::new();
    let settings = params(vec![3, 4]);
    scanner.start(EventSink::null(), target(port, Vec::new()), settings.clone())
        .await.expect("first scan starts");
    assert!(scanner.start(EventSink::null(), target(port, Vec::new()), settings.clone())
        .await.expect_err("concurrent start should fail").contains("already in progress"));

    let (mut stream, request) = recv_request(&listener).await;
    send_read_response(&mut stream, &request).await;
    scanner.control(ControlMsg::Pause).await.expect("pause scan");
    wait_status(&scanner, ScanStatus::Paused).await;

    assert!(scanner.start(EventSink::null(), target(port, Vec::new()), settings)
        .await.expect_err("paused scan still owns the engine").contains("already in progress"));
    scanner.control(ControlMsg::Stop).await.expect("stop paused scan");
    scanner.wait_finished(Duration::from_secs(2)).await.expect("stop completes");
    assert_eq!(scanner.status(), ScanStatus::Stopped);
    assert_no_more_connections(&listener).await;
}

#[tokio::test]
async fn stop_discards_queued_register_reads() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let scanner = ScannerHandle::new();
    let range = RegisterRange { function: 3, start: 10, quantity: 2 };
    scanner.start(EventSink::null(), target(port, vec![range]), params(vec![3]))
        .await.expect("start scan");

    let (mut stream, request) = recv_request(&listener).await;
    send_read_response(&mut stream, &request).await;
    // Ensure discovery has completed before exercising queued register work.
    tokio::time::timeout(Duration::from_secs(1), async {
        while scanner.results().records.is_empty() {
            tokio::task::yield_now().await;
        }
    }).await.expect("device discovered");
    tokio::time::sleep(Duration::from_millis(20)).await;

    // A 400ms inter-request gap allows Stop to arrive before the queued
    // register work. The old engine would still drain it after the stop.
    scanner.control(ControlMsg::Stop).await.expect("stop scan");
    scanner.wait_finished(Duration::from_secs(2)).await.expect("stop completes");
    assert_eq!(scanner.status(), ScanStatus::Stopped);
    assert_no_more_connections(&listener).await;
    assert!(scanner.results().records.iter().all(|r| r.registers.is_empty()));
}

#[tokio::test]
async fn stop_finishes_active_ip_probe_and_discards_full_concurrency_queue() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let scanner = ScannerHandle::new();
    let mut settings = params(vec![3, 4, 3]);
    settings.request_gap_ms = 0;
    settings.timeout_ms = 700;
    scanner.start(EventSink::null(), target(port, Vec::new()), settings)
        .await.expect("start scan");

    // The approved Stop contract now lets an already transmitted request
    // finish or time out, but never dispatches another queued operation.
    let (_stream, _request) = recv_request(&listener).await;
    scanner.control(ControlMsg::Stop).await.expect("stop scan");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(scanner.status(),ScanStatus::Stopping);
    scanner.wait_finished(Duration::from_secs(4)).await.expect("bounded safe completion");
    assert!(scanner.communication().manager.snapshot().is_empty());
    assert_eq!(scanner.status(), ScanStatus::Stopped);
    assert_no_more_connections(&listener).await;
}

#[tokio::test(flavor = "current_thread")]
async fn stop_before_spawned_engine_is_polled_never_opens_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let scanner = ScannerHandle::new();
    scanner.start(EventSink::null(), target(port, Vec::new()), params(vec![3]))
        .await.expect("start scan");
    scanner.control(ControlMsg::Stop).await.expect("stop immediately");
    scanner.wait_finished(Duration::from_secs(2)).await.expect("stop completion");
    assert_eq!(scanner.status(), ScanStatus::Stopped);
    assert_no_more_connections(&listener).await;
}

#[tokio::test]
async fn after_stop_the_handle_accepts_a_new_scan_and_rejects_stale_controls() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let scanner = ScannerHandle::new();
    scanner.start(EventSink::null(), target(port, Vec::new()), params(vec![3]))
        .await.expect("start");
    scanner.control(ControlMsg::Stop).await.expect("stop");
    scanner.wait_finished(Duration::from_secs(2)).await.expect("finished");
    // This result should not be blocked by a stale control sender.
    scanner.start(EventSink::null(), target(port, Vec::new()), params(vec![3]))
        .await.expect("restart after previous scan cleanup");
    scanner.control(ControlMsg::Stop).await.expect("stop restarted scan");
    scanner.wait_finished(Duration::from_secs(2)).await.expect("finished again");
    assert_eq!(scanner.status(), ScanStatus::Stopped);
    assert_no_more_connections(&listener).await;
}

#[tokio::test]
async fn scanner_and_monitor_respect_same_serial_registry_before_io(){
    use std::sync::Arc;
    use crate::communication::service::CommunicationService;
    use crate::scanner::monitor::MonitorHandle;
    use crate::scanner::types::{SerialPortParams,SerialSweepParams,SerialMode,MonitorWatch};
    use crate::transport::serial::{DataBitsSerde,ParitySerde,StopBitsSerde};
    let service=Arc::new(CommunicationService::default());
    let scanner=ScannerHandle::with_communication(service.clone());
    let monitor=MonitorHandle::with_communication(service.clone());
    let port=if cfg!(windows){"COM99"}else{"/dev/modbus-test-locked"};
    let lease=service.manager.lease_serial(port).unwrap();
    let settings=SerialPortParams{port_name:port.into(),baud_rate:9600,data_bits:DataBitsSerde::Eight,parity:ParitySerde::None,stop_bits:StopBitsSerde::One};
    let target=ScanTarget::UnitIdRange{from:1,to:1,port:settings.clone(),mode:SerialMode::Rtu};
    assert!(scanner.start(EventSink::null(),target,ScanParams::default()).await.unwrap_err().contains("already owned"));
    let target=ScanTarget::SerialAuto{sweep:SerialSweepParams::defaults(port.into(),SerialMode::Rtu,1,1)};
    assert!(scanner.start(EventSink::null(),target,ScanParams::default()).await.unwrap_err().contains("already owned"));
    let watch=MonitorWatch{key:"test".into(),target:DeviceTarget{kind:TransportKind::Rtu,endpoint:port.into(),port:None,unit_id:1,serial:Some(settings),serial_mode:Some(SerialMode::Rtu)},function:3,address:0,quantity:1,name:None,unit:None,tag_id:None,spec:None};
    assert!(monitor.start(EventSink::null(),vec![watch],250,Some(100)).await.unwrap_err().contains("already owned"));
    assert!(monitor.snapshot().is_empty());
    assert_eq!(scanner.status(),ScanStatus::Idle);
    drop(lease);
    assert!(service.manager.lease_serial(port).is_ok());
}

#[tokio::test]
async fn network_scan_serializes_same_endpoint_and_releases_all_workers(){
    let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port=listener.local_addr().unwrap().port();
    let scanner=ScannerHandle::new();
    let mut settings=params(vec![3,4,3,4]);
    settings.ip_concurrency=8;settings.request_gap_ms=0;
    let server=tokio::spawn(async move{
        for _ in 0..4{
            let (mut stream,request)=recv_request(&listener).await;
            assert!(tokio::time::timeout(Duration::from_millis(40),listener.accept()).await.is_err(),"second worker opened same endpoint");
            send_read_response(&mut stream,&request).await;
        }
    });
    scanner.start(EventSink::null(),target(port,Vec::new()),settings).await.unwrap();
    let result=scanner.wait_finished(Duration::from_secs(5)).await.unwrap();
    assert_eq!(result.stats.responses_ok,4);
    assert!(scanner.communication().manager.snapshot().is_empty());
    server.await.unwrap();
}
#[tokio::test]
async fn network_scan_cannot_open_endpoint_owned_by_manual_worker(){
    use crate::transport::connection::{ConnectionDefinition,TransportConfig};
    use crate::transport::tcp::TcpConfig;
    let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port=listener.local_addr().unwrap().port();
    let scanner=ScannerHandle::new();
    let manager=scanner.communication().manager.clone();
    let held=manager.temporary(ConnectionDefinition{id:"manual-held".into(),name:"Manual".into(),transport:TransportConfig::Tcp(TcpConfig{host:"127.0.0.1".into(),port,..Default::default()}),policy:Default::default()}).unwrap();
    scanner.start(EventSink::null(),target(port,Vec::new()),params(vec![3])).await.unwrap();
    let result=scanner.wait_finished(Duration::from_secs(3)).await.unwrap();
    assert!(result.records.is_empty());assert!(result.stats.transport_errors>0);
    assert_no_more_connections(&listener).await;
    assert_eq!(manager.snapshot().len(),1);
    held.close().await.unwrap();
}


mod managed_serial {
    use super::*;
    use std::sync::{Arc,Mutex};
    use std::sync::atomic::{AtomicUsize,Ordering};
    use crate::communication::service::CommunicationService;
    use crate::modbus::common::{SerialMode,UnitId};
    use crate::scanner::types::{SerialPortParams,SerialSweepParams};
    use crate::transport::connection::{ConnectionManager,ConnectionDefinition,TransportConfig,TransportFactory};
    use crate::transport::serial::{SerialPortConfig,DataBitsSerde,ParitySerde,StopBitsSerde};
    use crate::transport::{Transport,TransportError,TransportResult};
    use async_trait::async_trait;
    use tokio::sync::Semaphore;

    fn port()->&'static str{if cfg!(windows){"COM99"}else{"/dev/modbus-scanner-managed"}}
    fn settings()->SerialPortParams{SerialPortParams{port_name:port().into(),baud_rate:9600,data_bits:DataBitsSerde::Eight,parity:ParitySerde::None,stop_bits:StopBitsSerde::One}}
    fn definition(mode:SerialMode)->ConnectionDefinition{
        ConnectionDefinition{id:"manual-test".into(),name:"test".into(),transport:TransportConfig::Serial{config:SerialPortConfig{port_name:port().into(),..Default::default()},mode},policy:Default::default()}
    }
    struct Fixture {
        opens:AtomicUsize, sent:Mutex<Vec<(u32,Vec<u8>)>>,
        entered:Semaphore, release:Semaphore, gated:bool,
    }
    struct Wire { state:Arc<Fixture>, baud:u32,mode:SerialMode,last:Vec<u8> }
    struct Factory(Arc<Fixture>);
    #[async_trait]
    impl TransportFactory for Factory{
        async fn open(&self,c:&TransportConfig)->TransportResult<Box<dyn Transport>>{
            let TransportConfig::Serial{config,mode}=c else{return Err(TransportError::Other("serial expected".into()));};
            self.0.opens.fetch_add(1,Ordering::SeqCst);
            Ok(Box::new(Wire{state:self.0.clone(),baud:config.baud_rate,mode:*mode,last:Vec::new()}))
        }
    }
    #[async_trait]
    impl Transport for Wire{
        fn name(&self)->String{"managed serial fixture".into()}
        async fn reset(&mut self)->TransportResult<()>{Ok(())}
        async fn close(&mut self)->TransportResult<()>{Ok(())}
        async fn send_raw(&mut self,adu:&[u8])->TransportResult<()>{
            self.last=adu.to_vec();self.state.sent.lock().unwrap().push((self.baud,adu.to_vec()));Ok(())
        }
        async fn recv_raw(&mut self,_:usize)->TransportResult<Vec<u8>>{
            self.state.entered.add_permits(1);
            if self.state.gated{self.state.release.acquire().await.map_err(|_|TransportError::Cancelled)?.forget();}
            if self.baud!=9600{return Err(TransportError::Timeout(Duration::from_millis(50)));}
            let (unit,fc)=match self.mode{
                SerialMode::Rtu=> (self.last[0],self.last[1]),
                SerialMode::Ascii=>{let frame=crate::modbus::ascii::decode(&self.last)?;(frame.unit_id.0,frame.pdu.function())},
            };
            let pdu=if fc==0x2b{vec![0x2b,0x0e,1,1,0,0,1,0,3,b'A',b'B',b'C']}else{vec![fc,2,0,42]};
            Ok(match self.mode{SerialMode::Rtu=>crate::modbus::rtu::encode(UnitId(unit),&pdu),SerialMode::Ascii=>crate::modbus::ascii::encode(UnitId(unit),&pdu)})
        }
    }
    fn setup(gated:bool)->(ScannerHandle,Arc<ConnectionManager>,Arc<Fixture>){
        let state=Arc::new(Fixture{opens:AtomicUsize::new(0),sent:Mutex::new(Vec::new()),entered:Semaphore::new(0),release:Semaphore::new(0),gated});
        let m=Arc::new(ConnectionManager::with_factory(Arc::new(Factory(state.clone()))));
        (ScannerHandle::with_communication(Arc::new(CommunicationService{manager:m.clone()})),m,state)
    }
    #[tokio::test]
    async fn fixed_serial_scanner_reuses_existing_worker_across_units_and_fc43(){
        for mode in [SerialMode::Rtu,SerialMode::Ascii]{
            let (scanner,m,f)=setup(false);
            let held=m.acquire(definition(mode)).unwrap();
            let mut p=params(vec![3]);p.request_gap_ms=0;p.identify_devices=true;
            scanner.start(EventSink::null(),ScanTarget::UnitIdRange{from:1,to:2,port:settings(),mode},p).await.unwrap();
            let r=scanner.wait_finished(Duration::from_secs(5)).await.unwrap();
            assert_eq!(r.stats.responses_ok,2);
            assert_eq!(r.records.len(),2);
            assert!(r.records.iter().all(|row|row.device_identification.is_some()));
            assert_eq!(f.opens.load(Ordering::SeqCst),1);
            assert_eq!(f.sent.lock().unwrap().len(),4);
            assert_eq!(m.snapshot().len(),1);
            drop(held);m.shutdown().await.unwrap();
        }
    }
    #[tokio::test]
    async fn managed_serial_stop_waits_for_rx_and_does_not_start_next_unit(){
        let (scanner,m,f)=setup(true);
        let mut p=params(vec![3]);p.timeout_ms=3000;p.request_gap_ms=0;
        scanner.start(EventSink::null(),ScanTarget::UnitIdRange{from:1,to:2,port:settings(),mode:SerialMode::Rtu},p).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2),f.entered.acquire()).await.unwrap().unwrap().forget();
        scanner.control(ControlMsg::Stop).await.unwrap();
        wait_status(&scanner,ScanStatus::Stopping).await;
        assert_eq!(f.sent.lock().unwrap().len(),1);
        f.release.add_permits(1);
        scanner.wait_finished(Duration::from_secs(4)).await.unwrap();
        assert_eq!(scanner.status(),ScanStatus::Stopped);
        assert_eq!(f.sent.lock().unwrap().len(),1);
        assert_eq!(m.snapshot()[0].active_requests,0);
        m.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn serial_sweep_retires_idle_cache_and_recovers_between_settings(){
        let (scanner,m,f)=setup(false);
        let old=m.acquire(definition(SerialMode::Rtu)).unwrap();
        let old_id=old.id().to_owned();drop(old);
        let mut sweep=SerialSweepParams::defaults(port().into(),SerialMode::Rtu,1,1);
        sweep.baud_rates=vec![19200,9600];sweep.framings.truncate(1);sweep.discovery_unit_ids=vec![1];
        let mut p=params(vec![3]);p.request_gap_ms=0;p.timeout_ms=50;
        scanner.start(EventSink::null(),ScanTarget::SerialAuto{sweep},p).await.unwrap();
        let r=scanner.wait_finished(Duration::from_secs(5)).await.unwrap();
        assert!(r.records.iter().any(|row|row.target.serial.as_ref().is_some_and(|s|s.baud_rate==9600)));
        assert!(m.snapshot().is_empty());
        assert!(f.opens.load(Ordering::SeqCst)>=2);
        let sent=f.sent.lock().unwrap();assert_eq!(sent[0].0,19200);assert!(sent.iter().any(|(baud,_)|*baud==9600));drop(sent);
        let next=m.acquire(definition(SerialMode::Rtu)).unwrap();assert_ne!(next.id(),old_id);drop(next);
        m.shutdown().await.unwrap();
    }
}

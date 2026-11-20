//! Deterministic end-to-end Modbus TCP field scenarios.
//!
//! Uses a loopback fixture with real TCP sockets, the production scanner engine,
//! MBAP parsing, classification, and register-map logic. No OT equipment needed.
//! All listener ports are ephemeral and no traffic leaves localhost.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::modbus::common::TransportKind;
use super::engine::ScannerHandle;
use super::types::{DeviceTarget, IpTargetClass, ProbeResult, RegisterRange, ScanParams, ScanTarget, TrafficRecord};
use super::{EventSink, TrafficSink};

#[derive(Clone, Copy)]
enum Reply {
    Data,
    Exception(u8),
    WrongTransaction,
    WrongFunction,
    WrongExceptionFunction,
    ExtraExceptionBytes,
    InvalidByteCount,
    ShortRegisterData,
    Silent,
    Fragmented,
}

struct TestSlave {
    port: u16,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    arrivals: Arc<Mutex<Vec<Instant>>>,
    task: JoinHandle<()>,
}

async fn test_slave(replies: Vec<Reply>) -> TestSlave {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback fixture");
    let port = listener.local_addr().unwrap().port();
    let requests: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
    let observed = requests.clone();
    let arrivals: Arc<Mutex<Vec<Instant>>> = Arc::new(Mutex::new(Vec::new()));
    let times = arrivals.clone();
    let task = tokio::spawn(async move {
        for reply in replies {
            let (mut stream, _) = tokio::time::timeout(
                Duration::from_secs(5),
                listener.accept(),
            )
            .await
            .expect("scanner did not connect to fixture")
            .expect("accept loopback connection");

            let mut mbap = [0u8; 7];
            stream.read_exact(&mut mbap).await.expect("read MBAP");
            assert_eq!(u16::from_be_bytes([mbap[2], mbap[3]]), 0);
            let length = u16::from_be_bytes([mbap[4], mbap[5]]);
            assert!((2..=254).contains(&length), "invalid MBAP length {length}");
            let mut pdu = vec![0u8; length as usize - 1];
            stream.read_exact(&mut pdu).await.expect("read PDU");
            // The quiet-mode gate acts on physical request TX, not on
            // TCP connect(). Timestamp receipt of the complete Modbus PDU.
            times.lock().unwrap().push(Instant::now());
            let mut adu = mbap.to_vec();
            adu.extend_from_slice(&pdu);
            observed.lock().unwrap().push(adu);

            if matches!(reply, Reply::Silent) {
                // Connection stays open beyond the scan response timeout.
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue;
            }

            let function = pdu[0];
            let response_pdu = match reply {
                Reply::Exception(code) => vec![function | 0x80, code],
                Reply::WrongExceptionFunction => vec![(function + 1) | 0x80, 2],
                Reply::ExtraExceptionBytes => vec![function | 0x80, 2, 0],
                Reply::InvalidByteCount => vec![function, 4, 0, 42],
                Reply::ShortRegisterData => vec![function, 2, 0, 42],
                Reply::Data | Reply::WrongTransaction | Reply::WrongFunction | Reply::Fragmented => match function {
                    0x01 | 0x02 => vec![function, 1, 1],
                    0x2B => vec![function | 0x80, 0x01], // unsupported optional FC43
                    0x03 | 0x04 => {
                        assert!(pdu.len() >= 5, "short read request");
                        let quantity = u16::from_be_bytes([pdu[3], pdu[4]]);
                        assert!((1..=125).contains(&quantity));
                        let mut response = vec![function, (2 * quantity) as u8];
                        for i in 0..quantity {
                            response.extend_from_slice(&(0x002Au16 + i).to_be_bytes());
                        }
                        response
                    }
                    other => panic!("write or unsupported function in discovery: {other:#04x}"),
                },
                Reply::Silent => unreachable!(),
            };

            let mut response_pdu = response_pdu;
            if matches!(reply, Reply::WrongFunction) {
                response_pdu[0] = if response_pdu[0] == 0x03 { 0x04 } else { 0x03 };
            }
            let mut tx_id = u16::from_be_bytes([mbap[0], mbap[1]]);
            if matches!(reply, Reply::WrongTransaction) {
                tx_id = tx_id.wrapping_add(1);
            }
            let mut response = Vec::with_capacity(7 + response_pdu.len());
            response.extend_from_slice(&tx_id.to_be_bytes());
            response.extend_from_slice(&[0, 0]); // Protocol ID = Modbus
            response.extend_from_slice(&((1 + response_pdu.len()) as u16).to_be_bytes());
            response.push(mbap[6]); // unit ID
            response.extend_from_slice(&response_pdu);

            if matches!(reply, Reply::Fragmented) {
                // Exercise TCP fragmentation between MBAP and the register payload.
                stream.write_all(&response[..8]).await.expect("write partial response");
                tokio::time::sleep(Duration::from_millis(20)).await;
                stream.write_all(&response[8..]).await.expect("write remaining response");
            } else {
                stream.write_all(&response).await.expect("write response");
            }
        }
    });
    TestSlave { port, requests, arrivals, task }
}

fn single_tcp(port: u16, register_plan: Vec<RegisterRange>) -> ScanTarget {
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

fn safe_params(functions: Vec<u8>) -> ScanParams {
    ScanParams {
        timeout_ms: 100,
        request_gap_ms: 0,
        retries: 0,
        probe_functions: functions,
        identify_devices: false,
        ip_concurrency: 1,
        ip_allowlist: vec!["127.0.0.1/32".into()],
        ..ScanParams::default()
    }
}

async fn scan(target: ScanTarget, params: ScanParams) -> super::types::ScanResults {
    let handle = ScannerHandle::new();
    handle.start(EventSink::null(), target, params).await.expect("start scan");
    handle.wait_finished(Duration::from_secs(4)).await.expect("scan completion")
}

async fn observed(slave: TestSlave) -> Vec<Vec<u8>> {
    slave.task.await.expect("fixture completed without panic");
    slave.requests.lock().unwrap().clone()
}

#[tokio::test]
async fn readonly_discovery_fcs_are_01_02_03_04_only() {
    let fixture = test_slave(vec![Reply::Data; 4]).await;
    let requests = fixture.requests.clone();
    let results = scan(single_tcp(fixture.port, Vec::new()), safe_params(vec![1, 2, 3, 4])).await;
    observed(fixture).await;

    let sent = requests.lock().unwrap();
    assert_eq!(sent.len(), 4);
    assert_eq!(sent.iter().map(|adu| adu[7]).collect::<Vec<_>>(), [1, 2, 3, 4]);
    assert!(sent.iter().all(|adu| (1..=4).contains(&adu[7])));
    assert_eq!(results.stats.requests_sent, 4);
    assert_eq!(results.stats.responses_ok, 4);
    assert_eq!(results.records.len(), 1);
    assert_eq!(results.ip_targets[0].class, IpTargetClass::ValidModbus);
}

#[tokio::test]
async fn register_map_reads_real_mbap_and_fragmented_response() {
    let fixture = test_slave(vec![Reply::Data, Reply::Fragmented]).await;
    let target = single_tcp(fixture.port, vec![RegisterRange {
        function: 0x03, start: 10, quantity: 2,
    }]);
    let results = scan(target, safe_params(vec![0x03])).await;
    let requests = observed(fixture).await;

    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|adu| adu[7] == 0x03));
    assert_eq!(requests[1][8..], [0, 10, 0, 2]);
    assert_eq!(results.stats.responses_ok, 2);
    let regs = &results.records[0].registers;
    assert_eq!(regs["03:000a"].raw, 0x002A);
    assert_eq!(regs["03:000b"].raw, 0x002B);
}

#[tokio::test]
async fn modbus_exception_still_identifies_slave() {
    let fixture = test_slave(vec![Reply::Exception(0x02)]).await;
    let results = scan(single_tcp(fixture.port, Vec::new()), safe_params(vec![0x03])).await;
    observed(fixture).await;

    assert_eq!(results.stats.responses_exception, 1);
    assert_eq!(results.records.len(), 1);
    assert!(matches!(results.records[0].probes.get(&0x03), Some(ProbeResult::Exception { code: 2, .. })));
    assert_eq!(results.ip_targets[0].class, IpTargetClass::ValidModbus);
}

#[tokio::test]
async fn wrong_mbap_transaction_is_not_a_discovered_device() {
    let fixture = test_slave(vec![Reply::WrongTransaction]).await;
    let results = scan(single_tcp(fixture.port, Vec::new()), safe_params(vec![0x03])).await;
    observed(fixture).await;

    assert_eq!(results.stats.requests_sent, 1);
    assert_eq!(results.stats.transport_errors, 1);
    assert!(results.records.is_empty());
    assert_eq!(results.ip_targets[0].class, IpTargetClass::PortOpenNoModbus);
}

#[tokio::test]
async fn open_but_silent_tcp_port_is_not_a_discovered_device() {
    let fixture = test_slave(vec![Reply::Silent]).await;
    let results = scan(single_tcp(fixture.port, Vec::new()), safe_params(vec![0x03])).await;
    observed(fixture).await;

    assert_eq!(results.stats.requests_sent, 1);
    assert_eq!(results.stats.timeouts, 1);
    assert!(results.records.is_empty());
}

#[tokio::test]
async fn disallow_write_function_in_discovery_before_network_io() {
    let target = single_tcp(502, Vec::new());
    let handle = ScannerHandle::new();
    let error = handle.start(EventSink::null(), target, safe_params(vec![0x03, 0x06]))
        .await
        .expect_err("write function must not enter scan engine");
    assert!(error.contains("read-only"));
    assert_eq!(handle.status(), super::types::ScanStatus::Failed);
}

#[tokio::test]
async fn malformed_discovery_never_produces_a_false_device() {
    for reply in [
        Reply::InvalidByteCount,
        Reply::WrongFunction,
        Reply::WrongExceptionFunction,
        Reply::ExtraExceptionBytes,
    ] {
        let fixture = test_slave(vec![reply]).await;
        let port = fixture.port;
        let results = scan(single_tcp(port, Vec::new()), safe_params(vec![0x03])).await;
        let requests = observed(fixture).await;

        assert_eq!(requests.len(), 1, "unexpected retry of malformed FC03");
        assert!(results.records.is_empty(), "malformed data discovered a device");
        assert_eq!(results.stats.responses_ok, 0);
        assert_eq!(results.stats.responses_exception, 0);
        assert_eq!(results.stats.transport_errors, 1);
        assert_eq!(results.ip_targets[0].class, IpTargetClass::PortOpenNoModbus);
    }
}

#[tokio::test]
async fn malformed_register_map_response_is_never_stored_or_split() {
    let fixture = test_slave(vec![Reply::Data, Reply::ShortRegisterData]).await;
    let target = single_tcp(fixture.port, vec![RegisterRange {
        function: 0x03, start: 10, quantity: 2,
    }]);
    let results = scan(target, safe_params(vec![0x03])).await;
    let requests = observed(fixture).await;

    assert_eq!(requests.len(), 2, "malformed register read must not fan out");
    assert_eq!(results.stats.responses_ok, 1, "only discovery may succeed");
    assert_eq!(results.stats.transport_errors, 1);
    assert_eq!(results.records.len(), 1);
    assert!(results.records[0].registers.is_empty(), "corrupt values were persisted");
}

#[tokio::test]
async fn corrupt_initial_register_scan_does_not_split_into_more_requests() {
    use super::types::{RegisterScanMode, RegisterScanParams};

    let fixture = test_slave(vec![Reply::InvalidByteCount]).await;
    let target = ScanTarget::RegisterScan {
        scan: RegisterScanParams {
            target: DeviceTarget {
                kind: TransportKind::Tcp,
                endpoint: "127.0.0.1".into(),
                port: Some(fixture.port),
                unit_id: 1,
                serial: None,
                serial_mode: None,
            },
            function: 0x03,
            start: 0,
            end: 99,
            mode: RegisterScanMode::Balanced,
            batch: 100,
        },
    };
    let results = scan(target, safe_params(vec![0x03])).await;
    let requests = observed(fixture).await;

    assert_eq!(requests.len(), 1, "invalid PDU must abort adaptive scan");
    assert_eq!(results.stats.transport_errors, 1);
    assert!(results.records.is_empty());
}

#[derive(Default)]
struct CapturedTraffic {
    records: Mutex<Vec<TrafficRecord>>,
}

impl TrafficSink for CapturedTraffic {
    fn push(&self, record: TrafficRecord) {
        self.records.lock().unwrap().push(record);
    }
}

async fn scan_with_traffic(target: ScanTarget, params: ScanParams) -> (
    super::types::ScanResults,
    Vec<TrafficRecord>,
) {
    let collector = Arc::new(CapturedTraffic::default());
    let sink = EventSink::new(Arc::new(|_, _| {}), Some(collector.clone()), None);
    let handle = ScannerHandle::new();
    handle.start(sink, target, params).await.expect("start");
    let results = handle.wait_finished(Duration::from_secs(4)).await.expect("completion");
    let traffic = collector.records.lock().unwrap().clone();
    (results, traffic)
}

#[tokio::test]
async fn traffic_logs_are_invalid_for_corrupted_discovery_and_map_replies() {
    let invalid_discovery = test_slave(vec![Reply::InvalidByteCount]).await;
    let (results, traffic) = scan_with_traffic(
        single_tcp(invalid_discovery.port, Vec::new()),
        safe_params(vec![3]),
    ).await;
    observed(invalid_discovery).await;
    assert!(results.records.is_empty());
    assert_eq!(traffic.len(), 1);
    assert_eq!(traffic[0].status, "invalid");

    let invalid_map = test_slave(vec![Reply::Data, Reply::ShortRegisterData]).await;
    let (results, traffic) = scan_with_traffic(
        single_tcp(invalid_map.port, vec![RegisterRange {
            function: 3, start: 10, quantity: 2,
        }]),
        safe_params(vec![3]),
    ).await;
    observed(invalid_map).await;
    assert_eq!(traffic.len(), 2);
    assert_eq!(traffic[0].status, "ok");
    assert_eq!(traffic[1].status, "invalid");
    assert_eq!(results.stats.responses_ok, 1);
    assert!(results.records[0].registers.is_empty());
}

#[tokio::test]
async fn default_ip_allowlist_blocks_network_before_socket_open() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let scanner = ScannerHandle::new();
    let mut params = safe_params(vec![3]);
    params.ip_allowlist.clear();
    let error = scanner.start(EventSink::null(), single_tcp(port, Vec::new()), params)
        .await.expect_err("empty network allowlist must deny");
    assert!(error.contains("allowlist"), "{error}");
    assert!(tokio::time::timeout(Duration::from_millis(100), listener.accept()).await.is_err());
}

#[tokio::test]
async fn quiet_mode_rates_retries_and_fc43_at_two_total_requests_per_second() {
    // Responses are generated from incoming FC rather than assuming that
    // concurrently scheduled discovery and FC43 requests have a fixed order.
    let fixture = test_slave(vec![Reply::Data; 4]).await;
    let timestamps = fixture.arrivals.clone();
    let mut params = safe_params(vec![3, 4]);
    params.identify_devices = true;
    params.quiet_mode = true;
    params.ip_concurrency = 64;
    let results = scan(single_tcp(fixture.port, Vec::new()), params).await;
    let requests = observed(fixture).await;
    assert_eq!(requests.len(), 4);
    assert_eq!(results.stats.responses_ok, 2);
    let times = timestamps.lock().unwrap();
    for times in times.windows(2) {
        assert!(times[1].duration_since(times[0]) >= Duration::from_millis(430),
            "OT quiet mode sent two consecutive requests too quickly");
    }
}

#[tokio::test]
async fn quiet_mode_includes_retry_attempts() {
    let fixture = test_slave(vec![Reply::Silent, Reply::Data]).await;
    let timestamps = fixture.arrivals.clone();
    let mut params = safe_params(vec![3]);
    params.quiet_mode = true;
    params.retries = 1;
    params.timeout_ms = 100;
    let _ = scan(single_tcp(fixture.port, Vec::new()), params).await;
    let requests = observed(fixture).await;
    assert_eq!(requests.len(), 2);
    let times = timestamps.lock().unwrap();
    assert!(times[1].duration_since(times[0]) >= Duration::from_millis(430));
}

#[tokio::test]
async fn quiet_mode_paces_tcp_connection_attempts_even_when_no_modbus_reply_exists() {
    use super::types::IpTransportKind;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepted = tokio::spawn(async move {
        let mut times = Vec::new();
        for _ in 0..2 {
            let (stream, _) = tokio::time::timeout(
                Duration::from_secs(4), listener.accept()
            ).await.expect("quiet scanner never connected").unwrap();
            times.push(Instant::now());
            drop(stream); // connection gets no Modbus response
        }
        times
    });
    let mut params = safe_params(vec![3]);
    params.quiet_mode = true;
    params.ip_concurrency = 128; // ignored in quiet mode (effective 1)
    let target = ScanTarget::IpRange {
        from: "127.0.0.1".into(),
        to: None,
        port,
        kind: IpTransportKind::Tcp,
    };
    let results = scan(target, params).await;
    let times = accepted.await.unwrap();
    assert_eq!(times.len(), 2);
    assert!(times[1].duration_since(times[0]) >= Duration::from_millis(430),
        "Quiet Mode must pace TCP handshake attempts, even when no PDU is received");
    assert_eq!(results.stats.responses_ok, 0);
}

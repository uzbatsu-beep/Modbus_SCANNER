//! Lifecycle regression tests using an isolated TCP loopback listener.
//! Assert the absence of new outbound requests after Stop, not just UI status.

use std::time::{Duration, Instant};

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
async fn stop_interrupts_silent_ip_probe_and_full_concurrency_queue() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let scanner = ScannerHandle::new();
    let mut settings = params(vec![3, 4, 3]);
    settings.request_gap_ms = 0;
    settings.timeout_ms = 10_000;
    scanner.start(EventSink::null(), target(port, Vec::new()), settings)
        .await.expect("start scan");

    // Keep the connection alive but never respond. The second work item
    // waits on the full IP task pool, not on a timeout.
    let (_stream, _request) = recv_request(&listener).await;
    let started = Instant::now();
    scanner.control(ControlMsg::Stop).await.expect("stop scan");
    scanner.wait_finished(Duration::from_secs(2)).await.expect("fast cancellation");
    assert!(started.elapsed() < Duration::from_secs(2), "Stop waited for the long I/O timeout");
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

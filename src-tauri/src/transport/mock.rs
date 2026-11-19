//! In-memory transport for unit tests (E-08).
//!
//! Returns a queued sequence of responses and records every ADU that was sent,
//! so protocol/engine tests can run without a serial port, socket or WebView.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use super::{Transport, TransportError, TransportResult};

#[derive(Clone)]
pub struct MockTransport {
    responses: Arc<Mutex<VecDeque<TransportResult<Vec<u8>>>>>,
    pub sent: Arc<Mutex<Vec<Vec<u8>>>>,
    pub resets: Arc<AtomicUsize>,
    pub closed: Arc<AtomicBool>,
}

impl MockTransport {
    pub fn new(responses: Vec<TransportResult<Vec<u8>>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into())),
            sent: Arc::new(Mutex::new(Vec::new())),
            resets: Arc::new(AtomicUsize::new(0)),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl Transport for MockTransport {
    fn name(&self) -> String {
        "mock".into()
    }

    async fn reset(&mut self) -> TransportResult<()> {
        self.resets.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn close(&mut self) -> TransportResult<()> {
        self.closed.store(true, Ordering::Relaxed);
        Ok(())
    }

    async fn send_raw(&mut self, adu: &[u8]) -> TransportResult<()> {
        self.sent.lock().unwrap().push(adu.to_vec());
        Ok(())
    }

    async fn recv_raw(&mut self, _expected_hint: usize) -> TransportResult<Vec<u8>> {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(TransportError::Closed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn transact_sends_and_returns_queued_response() {
        let mut mock = MockTransport::new(vec![Ok(vec![0x01, 0x03, 0x02, 0x00, 0x2A, 0x00, 0x00])]);
        let result = mock
            .transact(&[0x01, 0x03, 0x00], Duration::from_millis(50), 8)
            .await
            .unwrap();
        assert_eq!(result[1], 0x03);
        assert_eq!(mock.sent.lock().unwrap().len(), 1);
        assert_eq!(mock.resets.load(Ordering::Relaxed), 1);
        mock.close().await.unwrap();
        assert!(mock.closed.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn empty_queue_reports_closed() {
        let mut mock = MockTransport::new(vec![]);
        let err = mock
            .transact(&[0x01], Duration::from_millis(50), 8)
            .await
            .unwrap_err();
        assert!(matches!(err, TransportError::Closed));
    }
}

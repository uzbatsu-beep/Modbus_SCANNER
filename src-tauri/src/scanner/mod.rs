//! Scanning engine.
//!
//! - `types`     - IPC contract types
//! - `strategies`- work-item generators
//! - `engine`    - scan task (transport-agnostic, no Tauri dependency)
//! - `monitor`   - live register polling
//! - `diff`      - compare two scan snapshots
//!
//! The engine never talks to Tauri directly: it emits through [`EventSink`],
//! which the Tauri boundary (see `bridge.rs`) implements. This keeps the core
//! testable and reusable from a future CLI/headless mode.

pub mod diff;
pub mod engine;
pub mod monitor;
pub mod safety;
pub mod strategies;
pub mod types;

#[cfg(test)]
mod field_tests;

#[cfg(test)]
mod lifecycle_tests;

use std::sync::Arc;

use types::{MonitorValueState, TrafficRecord};

/// Sink for raw Modbus exchanges (implemented by the app's traffic log).
pub trait TrafficSink: Send + Sync {
    fn push(&self, record: TrafficRecord);
}

/// Sink for monitor samples (implemented by the app's file loggers).
pub trait SampleSink: Send + Sync {
    fn write(&self, values: &[MonitorValueState]);
}

/// Transport for engine events. Payloads are JSON values so the core has no
/// dependency on Tauri or on the frontend's event names.
#[derive(Clone)]
pub struct EventSink {
    emit: Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>,
    traffic: Option<Arc<dyn TrafficSink>>,
    samples: Option<Arc<dyn SampleSink>>,
}

impl EventSink {
    pub fn new(
        emit: Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>,
        traffic: Option<Arc<dyn TrafficSink>>,
        samples: Option<Arc<dyn SampleSink>>,
    ) -> Self {
        Self {
            emit,
            traffic,
            samples,
        }
    }

    /// No-op sink for unit tests and headless runs without an event loop.
    pub fn null() -> Self {
        Self::new(Arc::new(|_, _| {}), None, None)
    }

    pub fn emit(&self, name: &str, payload: serde_json::Value) {
        (self.emit)(name, payload);
    }

    pub fn traffic(&self, record: TrafficRecord) {
        if let Some(sink) = &self.traffic {
            sink.push(record);
        }
    }

    pub fn samples(&self, values: &[MonitorValueState]) {
        if let Some(sink) = &self.samples {
            sink.write(values);
        }
    }
}

pub use engine::ScannerHandle;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn custom_sink_captures_events_without_tauri() {
        let count = Arc::new(AtomicUsize::new(0));
        let counter = count.clone();
        let sink = EventSink::new(
            Arc::new(move |name, _| {
                if name == "scan:event" {
                    counter.fetch_add(1, Ordering::Relaxed);
                }
            }),
            None,
            None,
        );
        sink.emit("scan:event", serde_json::json!({ "kind": "status_changed" }));
        assert_eq!(count.load(Ordering::Relaxed), 1);
        // A null sink must never panic.
        EventSink::null().emit("scan:event", serde_json::json!({}));
    }
}

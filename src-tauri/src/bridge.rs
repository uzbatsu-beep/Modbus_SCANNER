//! Tauri boundary: builds an [`EventSink`] from an `AppHandle`.
//!
//! This is the only place where the scanner core learns about Tauri. The engine,
//! monitor, strategies and protocol code stay framework-agnostic and can be
//! reused by a future CLI/headless mode with `EventSink::null()` or a custom sink.

use std::sync::Arc;

use tauri::AppHandle;

use crate::scanner::{EventSink, SampleSink, TrafficSink};

pub fn tauri_event_sink(
    app: AppHandle,
    traffic: Arc<dyn TrafficSink>,
    samples: Arc<dyn SampleSink>,
) -> EventSink {
    EventSink::new(
        Arc::new(move |name, payload| {
            use tauri::Emitter;
            let _ = app.emit(name, payload);
        }),
        Some(traffic),
        Some(samples),
    )
}

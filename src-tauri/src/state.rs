//! Shared application state (IPC-facing configuration store).

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::scanner::types::{now_ms, ScanParams, TagDefinition, TrafficRecord};

/// Shared application state, injected into Tauri as managed state.
pub struct AppState {
    /// Scanner engine handle
    pub scanner: crate::scanner::ScannerHandle,
    /// Live register monitor handle
    pub monitor: crate::scanner::monitor::MonitorHandle,
    /// Raw Modbus traffic log
    pub traffic: TrafficLogHandle,
    /// Configuration store
    pub config: parking_lot::Mutex<Config>,
}

/// Bounded raw-traffic log for protocol debugging (P3-06).
pub struct TrafficLogHandle {
    records: parking_lot::RwLock<Vec<TrafficRecord>>,
    next_id: AtomicU64,
}

impl Default for TrafficLogHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl TrafficLogHandle {
    pub fn new() -> Self {
        Self {
            records: parking_lot::RwLock::new(Vec::new()),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn push(&self, mut record: TrafficRecord) {
        record.id = self.next_id.fetch_add(1, Ordering::Relaxed);
        record.timestamp_ms = now_ms();
        let mut records = self.records.write();
        records.push(record);
        let overflow = records.len().saturating_sub(2000);
        if overflow > 0 {
            records.drain(..overflow);
        }
    }

    pub fn snapshot(&self) -> Vec<TrafficRecord> {
        self.records.read().clone()
    }

    pub fn clear(&self) {
        self.records.write().clear();
    }
}

/// Application configuration, persisted via `config` commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// General.
    pub language: String,
    pub theme: String,
    /// Default scan params (last used).
    pub scan_params: ScanParams,
    /// Last used serial port settings.
    pub last_serial: Option<crate::transport::serial::SerialPortConfig>,
    /// Last used TCP target.
    pub last_tcp_host: Option<String>,
    pub last_tcp_port: Option<u16>,
    /// Last used UDP target.
    pub last_udp_host: Option<String>,
    pub last_udp_port: Option<u16>,
    /// Export settings.
    pub export_dir: Option<String>,
    pub export_format: String,
    /// Named register tags (P3-03), persisted with the configuration.
    #[serde(default)]
    pub tags: Vec<TagDefinition>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: "ru".into(),
            theme: "dark".into(),
            scan_params: ScanParams::default(),
            last_serial: None,
            last_tcp_host: None,
            last_tcp_port: Some(502),
            last_udp_host: None,
            last_udp_port: Some(502),
            export_dir: None,
            export_format: "csv".into(),
            tags: Vec::new(),
        }
    }
}

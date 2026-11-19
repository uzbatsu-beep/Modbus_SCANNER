//! Shared application state (IPC-facing configuration store).

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::scanner::types::{
    now_ms, DeviceProfile, LogFormat, LoggerConfig, LoggerStatus, MonitorValueState,
    RotationMode, ScanParams, TagDefinition, TrafficRecord,
};

/// Shared application state, injected into Tauri as managed state.
pub struct AppState {
    /// Scanner engine handle (shared with the local API server)
    pub scanner: Arc<crate::scanner::ScannerHandle>,
    /// Local API server handle, if running
    pub api: parking_lot::Mutex<Option<crate::api::ApiServerHandle>>,
    /// Live register monitor handle
    pub monitor: crate::scanner::monitor::MonitorHandle,
    /// Raw Modbus traffic log
    pub traffic: Arc<TrafficLogHandle>,
    /// File loggers with rotation
    pub loggers: Arc<LoggerHandle>,
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

/// UTC date suffix `YYYYMMDD` from unix milliseconds (no extra dependency).
fn date_suffix(ms: u64) -> String {
    let days = (ms / 86_400_000) as i64;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}{m:02}{d:02}")
}

fn datetime_suffix(ms: u64) -> String {
    let secs = (ms / 1000) % 86_400;
    format!(
        "{}_{:02}{:02}{:02}",
        date_suffix(ms),
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

fn rotated_path(base: &str, rotation: &RotationMode, started_ms: u64, now: u64, seq: u64) -> String {
    let (stem, ext) = match base.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (base.to_string(), String::new()),
    };
    match rotation {
        RotationMode::None => base.to_string(),
        RotationMode::Daily => format!("{stem}_{}{ext}", date_suffix(now)),
        RotationMode::Session => format!("{stem}_{}{ext}", datetime_suffix(started_ms)),
        RotationMode::SizeMb(limit) => {
            let path = Path::new(base);
            let oversized = path
                .metadata()
                .map(|m| m.len() > (*limit as u64) * 1024 * 1024)
                .unwrap_or(false);
            if oversized {
                format!("{stem}_{}_{seq}{ext}", datetime_suffix(now))
            } else {
                base.to_string()
            }
        }
    }
}

struct LoggerState {
    config: LoggerConfig,
    started_ms: u64,
    records_written: u64,
    last_write_ms: Option<u64>,
    error: Option<String>,
    current_file: String,
}

/// Multiple file loggers consuming monitor samples (P3-07).
pub struct LoggerHandle {
    loggers: parking_lot::RwLock<HashMap<String, LoggerState>>,
}

impl Default for LoggerHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl LoggerHandle {
    pub fn new() -> Self {
        Self {
            loggers: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    pub fn set_loggers(&self, configs: Vec<LoggerConfig>) {
        let now = now_ms();
        let mut loggers = self.loggers.write();
        loggers.clear();
        for config in configs {
            loggers.insert(
                config.id.clone(),
                LoggerState {
                    config,
                    started_ms: now,
                    records_written: 0,
                    last_write_ms: None,
                    error: None,
                    current_file: String::new(),
                },
            );
        }
    }

    pub fn status(&self) -> Vec<LoggerStatus> {
        let mut out: Vec<_> = self
            .loggers
            .read()
            .values()
            .map(|s| LoggerStatus {
                id: s.config.id.clone(),
                path: s.config.path.clone(),
                format: s.config.format,
                rotation: s.config.rotation.clone(),
                enabled: s.config.enabled,
                records_written: s.records_written,
                current_file: s.current_file.clone(),
                last_write_ms: s.last_write_ms,
                error: s.error.clone(),
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Write one batch of monitor samples to every enabled logger.
    pub fn write(&self, values: &[MonitorValueState]) {
        let mut loggers = self.loggers.write();
        for state in loggers.values_mut() {
            if !state.config.enabled {
                continue;
            }
            let selected: Vec<_> = values
                .iter()
                .filter(|v| {
                    state.config.keys.is_empty() || state.config.keys.contains(&v.key)
                })
                .collect();
            if selected.is_empty() {
                continue;
            }

            let now = now_ms();
            let file = rotated_path(
                &state.config.path,
                &state.config.rotation,
                state.started_ms,
                now,
                state.records_written,
            );
            let is_new = !Path::new(&file).exists();
            let mut rows = Vec::new();
            if is_new && matches!(state.config.format, LogFormat::Csv | LogFormat::Tsv) {
                let sep = if state.config.format == LogFormat::Csv { ";" } else { "\t" };
                rows.push(
                    ["timestamp_ms", "key", "name", "index", "value", "rtt_us", "errors"].join(sep),
                );
            }
            for value in selected {
                for sample in value.samples.iter().rev().take(1) {
                    for (i, raw) in sample.values.iter().enumerate() {
                        match state.config.format {
                            LogFormat::Csv | LogFormat::Tsv => {
                                let sep = if state.config.format == LogFormat::Csv { ";" } else { "\t" };
                                rows.push(
                                    [
                                        sample.timestamp_ms.to_string(),
                                        value.key.clone(),
                                        value.name.clone(),
                                        raw.to_string(),
                                        sample.rtt_us.to_string(),
                                        format!("idx{i}"),
                                        value.errors.to_string(),
                                    ]
                                    .join(sep),
                                );
                            }
                            LogFormat::Json => {
                                let row = serde_json::json!({
                                    "timestamp_ms": sample.timestamp_ms,
                                    "key": value.key,
                                    "name": value.name,
                                    "index": i,
                                    "value": raw,
                                    "rtt_us": sample.rtt_us,
                                    "errors": value.errors,
                                });
                                rows.push(row.to_string());
                            }
                        }
                    }
                }
            }

            let result = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file)
                .and_then(|mut f| {
                    for row in rows {
                        writeln!(f, "{row}")?;
                    }
                    Ok(())
                });
            match result {
                Ok(()) => {
                    state.records_written += 1;
                    state.last_write_ms = Some(now);
                    state.current_file = file;
                    state.error = None;
                }
                Err(e) => state.error = Some(e.to_string()),
            }
        }
    }
}

impl crate::scanner::TrafficSink for TrafficLogHandle {
    fn push(&self, record: TrafficRecord) {
        TrafficLogHandle::push(self, record);
    }
}

impl crate::scanner::SampleSink for LoggerHandle {
    fn write(&self, values: &[MonitorValueState]) {
        LoggerHandle::write(self, values);
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
    /// Reusable device profiles (E-04), persisted with the configuration.
    #[serde(default)]
    pub profiles: Vec<DeviceProfile>,
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
            profiles: Vec::new(),
        }
    }
}

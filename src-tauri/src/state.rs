//! Shared application state (IPC-facing configuration store).

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::mpsc::{sync_channel, SyncSender};
use crate::scanner::measurement::LogObservation;
use crate::modbus::decode::DecodedValue;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::scanner::types::{
    now_ms, DeviceProfile, LogFormat, LoggerConfig, LoggerStatus,
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

const LOGGER_QUEUE: usize = 4096;
const LOG_HEADER: &str = "schema_version;session_id;sequence;timestamp_ms;elapsed_us;watch_id;tag_id;name;transport;endpoint;unit_id;function;address;raw_hex;value;unit;quality;exception_code;detail";

fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn log_fields(record: &LogObservation) -> Vec<String> {
    let w = &record.watch;
    let s = &record.sample;
    let raw = s.raw.as_ref().map(|words| words.iter().map(|v| format!("{v:04X}")).collect::<Vec<_>>().join(" ")).unwrap_or_default();
    let value = match &s.value {
        Some(DecodedValue::Integer { value }) => value.to_string(),
        Some(DecodedValue::Float { value }) => value.to_string(),
        Some(DecodedValue::Text { value }) => value.clone(),
        None => String::new(),
    };
    let quality = serde_json::to_value(s.quality).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
    vec!["2".into(), record.session_id.clone(), s.sequence.to_string(), s.timestamp_ms.to_string(), s.elapsed_us.to_string(),
        w.key.clone(), w.tag_id.clone().unwrap_or_default(), w.name.clone().unwrap_or_default(),
        format!("{:?}", w.target.kind), format!("{}{}", w.target.endpoint, w.target.port.map(|p| format!(":{p}")).unwrap_or_default()),
        w.target.unit_id.to_string(), w.function.to_string(), w.address.to_string(), raw, value,
        w.unit.clone().unwrap_or_default(), quality, s.exception_code.map(|v| format!("0x{v:02X}")).unwrap_or_default(),
        s.detail.clone().unwrap_or_default()]
}

fn log_path(config: &LoggerConfig, record: &LogObservation, index: u64) -> String {
    let path = Path::new(&config.path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("modbus-values");
    let extension = match config.format { LogFormat::Csv => "csv", LogFormat::Tsv => "tsv", LogFormat::Json => "jsonl" };
    let suffix = match &config.rotation {
        RotationMode::None => String::new(),
        RotationMode::Daily => format!("_{}", date_suffix(record.sample.timestamp_ms)),
        RotationMode::Session => format!("_{}", record.session_id),
        RotationMode::SizeMb(_) => format!("_{}_{}", record.session_id, index),
    };
    path.with_file_name(format!("{stem}.v2{suffix}.{extension}")).to_string_lossy().into_owned()
}

struct LoggerState {
    config: LoggerConfig,
    records_written: u64,
    failed_records: u64,
    last_write_ms: Option<u64>,
    error: Option<String>,
    current_file: String,
    rotation_index: u64,
    rotation_session: String,
}

struct LogQueue {
    loggers: parking_lot::RwLock<HashMap<String, LoggerState>>,
    generation: AtomicU64,
    enabled: std::sync::atomic::AtomicBool,
    pending: AtomicU64,
    dropped: AtomicU64,
}

/// File IO is isolated on one worker; producers never wait for disk.
pub struct LoggerHandle {
    shared: Arc<LogQueue>,
    tx: SyncSender<(u64, LogObservation)>,
}

impl Default for LoggerHandle {
    fn default() -> Self { Self::new() }
}

impl LoggerHandle {
    pub fn new() -> Self {
        let shared = Arc::new(LogQueue {
            loggers: parking_lot::RwLock::new(HashMap::new()),
            generation: AtomicU64::new(0), enabled: std::sync::atomic::AtomicBool::new(false),
            pending: AtomicU64::new(0), dropped: AtomicU64::new(0),
        });
        let (tx, rx) = sync_channel::<(u64, LogObservation)>(LOGGER_QUEUE);
        let worker = shared.clone();
        std::thread::spawn(move || {
            while let Ok((generation, record)) = rx.recv() {
                let mut loggers = worker.loggers.write();
                if generation != worker.generation.load(Ordering::Acquire) {
                    worker.dropped.fetch_add(1, Ordering::Relaxed);
                } else {
                    for state in loggers.values_mut() {
                        if !state.config.enabled || (!state.config.keys.is_empty() && !state.config.keys.contains(&record.watch.key)) { continue; }
                        if let Err(e) = write_observation(state, &record) {
                            state.failed_records += 1;
                            state.error = Some(e.to_string());
                        }
                    }
                }
                worker.pending.fetch_sub(1, Ordering::Relaxed);
            }
        });
        Self { shared, tx }
    }

    pub fn set_loggers(&self, configs: Vec<LoggerConfig>) {
        let mut loggers = self.shared.loggers.write();
        self.shared.enabled.store(false, Ordering::Release);
        self.shared.generation.fetch_add(1, Ordering::AcqRel);
        loggers.clear();
        for config in configs {
            loggers.insert(config.id.clone(), LoggerState {
                config, records_written: 0, failed_records: 0, last_write_ms: None,
                error: None, current_file: String::new(), rotation_index: 0, rotation_session: String::new(),
            });
        }
        self.shared.enabled.store(loggers.values().any(|s| s.config.enabled), Ordering::Release);
    }

    pub fn status(&self) -> Vec<LoggerStatus> {
        let mut out: Vec<_> = self.shared.loggers.read().values().map(|s| LoggerStatus {
            id: s.config.id.clone(), path: s.config.path.clone(), format: s.config.format,
            rotation: s.config.rotation.clone(), enabled: s.config.enabled, records_written: s.records_written,
            dropped_records: self.shared.dropped.load(Ordering::Relaxed),
            pending_records: self.shared.pending.load(Ordering::Relaxed), failed_records: s.failed_records,
            current_file: s.current_file.clone(), last_write_ms: s.last_write_ms, error: s.error.clone(),
        }).collect();
        out.sort_by(|a,b| a.id.cmp(&b.id));
        out
    }

    pub fn write(&self, records: &[LogObservation]) {
        if !self.shared.enabled.load(Ordering::Acquire) { return; }
        let generation = self.shared.generation.load(Ordering::Acquire);
        for record in records {
            self.shared.pending.fetch_add(1, Ordering::Relaxed);
            if self.tx.try_send((generation, record.clone())).is_err() {
                self.shared.pending.fetch_sub(1, Ordering::Relaxed);
                self.shared.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

fn write_observation(state: &mut LoggerState, record: &LogObservation) -> std::io::Result<()> {
    if state.rotation_session != record.session_id {
        state.rotation_session = record.session_id.clone();
        state.rotation_index = 0;
    }
    let mut path = log_path(&state.config, record, state.rotation_index);
    if let RotationMode::SizeMb(limit) = &state.config.rotation {
        let limit = (*limit as u64).max(1) * 1024 * 1024;
        while Path::new(&path).metadata().map(|m| m.len() >= limit).unwrap_or(false) {
            state.rotation_index += 1;
            path = log_path(&state.config, record, state.rotation_index);
        }
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    if matches!(state.config.format, LogFormat::Csv | LogFormat::Tsv) {
        let sep = if state.config.format == LogFormat::Csv { ";" } else { "\t" };
        if file.metadata()?.len() == 0 { writeln!(file, "{}", LOG_HEADER.replace(';', sep))?; }
        let row = log_fields(record).iter().map(|s| quoted(s)).collect::<Vec<_>>().join(sep);
        writeln!(file, "{row}")?;
    } else {
        let row = serde_json::json!({"schema_version":2,"session_id":record.session_id,"watch":record.watch,"sample":record.sample});
        writeln!(file, "{row}")?;
    }
    file.flush()?;
    state.records_written += 1;
    state.last_write_ms = Some(now_ms());
    state.current_file = path;
    state.error = None;
    Ok(())
}

impl crate::scanner::TrafficSink for TrafficLogHandle {
    fn push(&self, record: TrafficRecord) {
        TrafficLogHandle::push(self, record);
    }
}

impl crate::scanner::SampleSink for LoggerHandle {
    fn write(&self, values: &[LogObservation]) {
        LoggerHandle::write(self, values);
    }
}

/// Application configuration, persisted via `config` commands.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
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

#[cfg(test)]
mod logger_v2_tests {
    use super::*;
    use crate::scanner::types::{DataQuality, DeviceTarget, MonitorSample, MonitorWatch};
    use crate::modbus::common::TransportKind;
    fn record(quality: DataQuality, sequence: u64) -> LogObservation {
        LogObservation { session_id: "session".into(), watch: MonitorWatch {
            key:"tag:a".into(), target:DeviceTarget { kind:TransportKind::Tcp, endpoint:"localhost".into(), port:Some(502), unit_id:1, serial:None, serial_mode:None },
            function:3,address:8193,quantity:1,name:Some("a;\"b".into()),unit:Some("Hz".into()),tag_id:Some("a".into()),spec:None,
        }, sample: MonitorSample { sequence,timestamp_ms:100,elapsed_us:100,raw:(quality==DataQuality::Good).then(||vec![0xec78]),
            value:(quality==DataQuality::Good).then_some(DecodedValue::Float{value:-50.0}),quality,exception_code:None,detail:None } }
    }
    #[test]
    fn csv_fields_match_and_timeout_has_no_value() {
        let good=log_fields(&record(DataQuality::Good,1));
        let bad=log_fields(&record(DataQuality::Timeout,2));
        assert_eq!(good.len(),LOG_HEADER.split(';').count());
        assert_eq!(good[13],"EC78");
        assert_eq!(good[14],"-50");
        assert_eq!(bad[13],"");
        assert_eq!(bad[14],"");
        assert_eq!(bad[16],"timeout");
        assert_eq!(quoted(&good[7]),"\"a;\"\"b\"");
    }
    #[test]
    fn separate_v2_paths_and_session_rotation() {
        let config=LoggerConfig{id:"a".into(),path:"old.csv".into(),format:LogFormat::Csv,rotation:RotationMode::Session,keys:vec![],enabled:true};
        assert_eq!(log_path(&config,&record(DataQuality::Good,1),0),"old.v2_session.csv");
    }
    #[test]
    fn writer_records_each_observation_once() {
        let path=std::env::temp_dir().join(format!("modbus-log-test-{:032x}.csv",rand::random::<u128>()));
        let config=LoggerConfig{id:"a".into(),path:path.to_string_lossy().into_owned(),format:LogFormat::Csv,rotation:RotationMode::None,keys:vec![],enabled:true};
        let mut state=LoggerState{config,records_written:0,failed_records:0,last_write_ms:None,error:None,current_file:String::new(),rotation_index:0,rotation_session:String::new()};
        write_observation(&mut state,&record(DataQuality::Good,1)).unwrap();
        write_observation(&mut state,&record(DataQuality::Timeout,2)).unwrap();
        let text=std::fs::read_to_string(&state.current_file).unwrap();
        assert_eq!(text.lines().count(),3);
        assert_eq!(text.matches("EC78").count(),1);
        assert_eq!(state.records_written,2);
        std::fs::remove_file(&state.current_file).unwrap();
    }
}

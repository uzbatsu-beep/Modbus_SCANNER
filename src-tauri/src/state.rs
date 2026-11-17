//! Shared application state (IPC-facing configuration store).

use serde::{Deserialize, Serialize};

use crate::scanner::types::ScanParams;

/// Shared application state, injected into Tauri as managed state.
pub struct AppState {
    /// Scanner engine handle
    pub scanner: crate::scanner::ScannerHandle,
    /// Configuration store
    pub config: parking_lot::Mutex<Config>,
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
        }
    }
}

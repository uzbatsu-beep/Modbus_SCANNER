//! System commands: app info, log level.

use serde::Serialize;

#[derive(Serialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub rustc: &'static str,
    pub debug: bool,
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        rustc: "1.77+",
        debug: cfg!(debug_assertions),
    }
}

#[tauri::command]
pub fn get_log_level() -> String {
    std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into())
}

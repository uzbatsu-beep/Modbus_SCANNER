//! System commands: app info, log level.

use serde::Serialize;
use tauri::State;

use crate::state::AppState;

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

/// Start the local REST/WebSocket API from the desktop app (E-03).
#[tauri::command]
pub async fn start_api_server(state: State<'_, AppState>, port: u16) -> Result<(), String> {
    if state.api.lock().is_some() {
        return Err("local API server is already running".into());
    }
    let handle = crate::api::start(
        port,
        state.scanner.clone(),
        state.traffic.clone(),
        state.loggers.clone(),
    )
    .await?;
    *state.api.lock() = Some(handle);
    Ok(())
}

#[tauri::command]
pub fn stop_api_server(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(mut handle) = state.api.lock().take() {
        handle.stop();
        Ok(())
    } else {
        Err("local API server is not running".into())
    }
}

#[tauri::command]
pub fn get_api_status(state: State<'_, AppState>) -> Option<u16> {
    state.api.lock().as_ref().map(|handle| handle.port)
}

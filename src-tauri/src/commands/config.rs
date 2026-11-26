//! Config commands: load/save the application configuration.
//!
//! Config is stored as JSON in the standard Tauri app config dir:
//! - Windows: `%APPDATA%/com.modbus-scanner.app/config-v2.json`
//! - Linux:   `~/.config/com.modbus-scanner.app/config-v2.json`
//! - macOS:   `~/Library/Application Support/com.modbus-scanner.app/config-v2.json`

use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use crate::state::{AppState, Config};

pub fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("config dir: {e}"))?;
    Ok(dir.join("config-v2.json"))
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    Ok(state.config.lock().clone())
}

/// Persist a config to disk (shared with project loading).
pub fn persist_config(app: &AppHandle, config: &Config) -> Result<(), String> {
    crate::scanner::types::validate_tags(&config.tags)?;
    let path = config_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn save_config(app: AppHandle, state: State<'_, AppState>, config: Config) -> Result<(), String> {
    persist_config(&app, &config)?;
    *state.config.lock() = config;
    Ok(())
}

#[tauri::command]
pub fn load_config(app: AppHandle, state: State<'_, AppState>) -> Result<Config, String> {
    let path = config_path(&app)?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let config: Config = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    crate::scanner::types::validate_tags(&config.tags)?;
    *state.config.lock() = config.clone();
    Ok(config)
}

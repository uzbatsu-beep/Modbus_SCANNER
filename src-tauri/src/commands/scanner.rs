//! Scanner lifecycle commands: start/stop/pause/resume, status, results, export.

use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::scanner::engine::{ControlMsg, ScannerHandle};
use crate::scanner::types::{ScanParams, ScanProgress, ScanResults, ScanStatus, ScanTarget};
use crate::state::AppState;

#[tauri::command]
pub async fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    target: ScanTarget,
    params: ScanParams,
) -> Result<(), String> {
    state.scanner.start(app, target, params).await
}

#[tauri::command]
pub async fn stop_scan(state: State<'_, AppState>) -> Result<(), String> {
    state.scanner.control(ControlMsg::Stop).await
}

#[tauri::command]
pub async fn pause_scan(state: State<'_, AppState>) -> Result<(), String> {
    state.scanner.control(ControlMsg::Pause).await
}

#[tauri::command]
pub async fn resume_scan(state: State<'_, AppState>) -> Result<(), String> {
    state.scanner.control(ControlMsg::Resume).await
}

#[tauri::command]
pub fn get_scan_status(state: State<'_, AppState>) -> ScanStatus {
    state.scanner.status()
}

#[tauri::command]
pub fn get_scan_progress(state: State<'_, AppState>) -> ScanProgress {
    state.scanner.progress()
}

#[tauri::command]
pub fn get_scan_results(state: State<'_, AppState>) -> ScanResults {
    state.scanner.results()
}

#[tauri::command]
pub fn clear_results(state: State<'_, AppState>) -> Result<(), String> {
    state.scanner.clear_results();
    Ok(())
}

/// Export format.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
    Xml,
}

/// Export the current scan results to a file.
#[tauri::command]
pub async fn export_results(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    format: ExportFormat,
) -> Result<usize, String> {
    let results = state.scanner.results();

    let data = match format {
        ExportFormat::Csv => results_to_csv(&results).map_err(|e| e.to_string())?,
        ExportFormat::Json => serde_json::to_vec_pretty(&results).map_err(|e| e.to_string())?,
        ExportFormat::Xml => results_to_xml(&results).map_err(|e| e.to_string())?,
    };

    let count = results.records.len();
    let path = PathBuf::from(&path);
    tokio::fs::write(&path, data).await.map_err(|e| e.to_string())?;

    let _ = app;
    Ok(count)
}

fn results_to_csv(results: &ScanResults) -> Result<Vec<u8>, String> {
    use std::fmt::Write;
    let mut w = String::new();
    writeln!(w, "key;transport;endpoint;port;unit_id;first_seen;last_seen;function;status;values")
        .map_err(|e| e.to_string())?;
    for rec in &results.records {
        for (fc, probe) in &rec.probes {
            let status = match probe {
                crate::scanner::types::ProbeResult::Ok { .. } => "ok",
                crate::scanner::types::ProbeResult::Exception { .. } => "exception",
                crate::scanner::types::ProbeResult::NoResponse { .. } => "no_response",
                crate::scanner::types::ProbeResult::TransportError { .. } => "transport_error",
            };
            writeln!(
                w,
                "{};{};{};{};{};{};{};{:#04x};{};",
                rec.key,
                format!("{:?}", rec.target.kind).to_lowercase(),
                rec.target.endpoint,
                rec.target.port.map(|p| p.to_string()).unwrap_or_default(),
                rec.target.unit_id,
                rec.first_seen_ms,
                rec.last_seen_ms,
                fc,
                status,
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(w.into_bytes())
}

fn results_to_xml(results: &ScanResults) -> Result<Vec<u8>, String> {
    fn escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }
    let mut w = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<scan_results>\n");
    for rec in &results.records {
        w.push_str(&format!(
            "  <device key=\"{}\" transport=\"{}\" endpoint=\"{}\" unit_id=\"{}\" first_seen=\"{}\" last_seen=\"{}\">\n",
            escape(&rec.key),
            escape(&format!("{:?}", rec.target.kind).to_lowercase()),
            escape(&rec.target.endpoint),
            rec.target.unit_id,
            rec.first_seen_ms,
            rec.last_seen_ms
        ));
        w.push_str("  </device>\n");
    }
    w.push_str("</scan_results>\n");
    Ok(w.into_bytes())
}

/// Keep the handle import even if a command signature changes.
#[allow(dead_code)]
fn _handle_used(_h: &ScannerHandle) {}

//! Scanner lifecycle commands: start/stop/pause/resume, status, results, export.

use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::scanner::engine::{ControlMsg, ScannerHandle};
use crate::scanner::types::{
    now_ms, DeviceRecord, MonitorValueState, MonitorWatch, ProbeResult, Project, ScanParams,
    ScanProgress, ScanResults, ScanSnapshot, ScanStatus, ScanTarget,
};
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

/// Start live polling of selected registers (P3-05).
#[tauri::command]
pub async fn start_monitor(
    app: AppHandle,
    state: State<'_, AppState>,
    watches: Vec<MonitorWatch>,
    interval_ms: u32,
) -> Result<(), String> {
    state.monitor.start(app, watches, interval_ms).await
}

#[tauri::command]
pub async fn stop_monitor(state: State<'_, AppState>) -> Result<(), String> {
    state.monitor.stop().await
}

#[tauri::command]
pub fn get_monitor_state(state: State<'_, AppState>) -> Vec<MonitorValueState> {
    state.monitor.snapshot()
}

/// Save the current session, tag map and scan history into one project file (P3-04).
#[tauri::command]
pub async fn save_project(
    state: State<'_, AppState>,
    path: String,
    name: String,
) -> Result<String, String> {
    let results = state.scanner.results();
    let tags = state.config.lock().tags.clone();
    let now = now_ms();
    let file = PathBuf::from(&path);

    // Keep the existing history when overwriting a project file.
    let existing: Option<Project> = if file.exists() {
        let data = tokio::fs::read(&file).await.map_err(|e| e.to_string())?;
        serde_json::from_slice(&data).ok()
    } else {
        None
    };
    let created_ms = existing.as_ref().map(|p| p.created_ms).unwrap_or(now);
    let mut history: Vec<ScanSnapshot> = existing.map(|p| p.history).unwrap_or_default();

    let mut snapshot_results = results.clone();
    if snapshot_results.stats.finished_ms.is_none() {
        snapshot_results.stats.finished_ms = Some(now);
    }
    history.push(ScanSnapshot {
        timestamp_ms: now,
        label: format!("{} — {}", name, results.stats.requests_sent),
        results: snapshot_results,
    });
    if history.len() > 20 {
        history.drain(..history.len() - 20);
    }

    let project = Project {
        name,
        version: 1,
        created_ms,
        updated_ms: now,
        results,
        tags,
        history,
    };
    let data = serde_json::to_vec_pretty(&project).map_err(|e| e.to_string())?;
    tokio::fs::write(&file, data).await.map_err(|e| e.to_string())?;
    Ok(path)
}

/// Load a project: results, tag map and history; also syncs config.json.
#[tauri::command]
pub async fn load_project(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Project, String> {
    let data = tokio::fs::read(PathBuf::from(&path))
        .await
        .map_err(|e| e.to_string())?;
    let project: Project = serde_json::from_slice(&data).map_err(|e| e.to_string())?;

    state.scanner.set_results(project.results.clone());
    {
        let mut config = state.config.lock();
        config.tags = project.tags.clone();
    }
    let config = state.config.lock().clone();
    crate::commands::config::persist_config(&app, &config)?;
    Ok(project)
}

/// Export a single selected device (P2-08) in the chosen format.
#[tauri::command]
pub async fn export_device(
    state: State<'_, AppState>,
    key: String,
    path: String,
    format: ExportFormat,
) -> Result<bool, String> {
    let results = state.scanner.results();
    let record = results
        .records
        .iter()
        .find(|r| r.key == key)
        .cloned()
        .ok_or_else(|| format!("device not found: {key}"))?;

    let data = match format {
        ExportFormat::Csv => device_to_csv(&record).map_err(|e| e.to_string())?,
        ExportFormat::Json => serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
        ExportFormat::Xml => device_to_xml(&record).map_err(|e| e.to_string())?,
    };
    tokio::fs::write(PathBuf::from(&path), data).await.map_err(|e| e.to_string())?;
    Ok(true)
}

fn csv_cell(value: &str) -> String {
    if value.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn device_identity(rec: &DeviceRecord) -> (String, String, String) {
    match &rec.device_identification {
        Some(id) => (
            id.vendor_name().unwrap_or("").to_string(),
            id.product_code().unwrap_or("").to_string(),
            id.revision().unwrap_or("").to_string(),
        ),
        None => (String::new(), String::new(), String::new()),
    }
}

fn device_to_csv(rec: &DeviceRecord) -> Result<Vec<u8>, String> {
    use std::fmt::Write;
    let (vendor, product, revision) = device_identity(rec);
    let mut w = String::new();
    writeln!(
        w,
        "section;key;transport;endpoint;port;unit_id;vendor;product;revision;function;address;status;value;raw_hex;rtt_us"
    )
    .map_err(|e| e.to_string())?;

    let mut probes: Vec<_> = rec.probes.iter().collect();
    probes.sort_by_key(|(fc, _)| **fc);
    for (fc, probe) in probes {
        let (status, value, raw, rtt) = match probe {
            ProbeResult::Ok { values, raw_hex, rtt_us, .. } => (
                "ok",
                values.iter().map(|v| format!("{v:04x}")).collect::<Vec<_>>().join(" "),
                raw_hex.clone(),
                *rtt_us,
            ),
            ProbeResult::Exception { code, message, rtt_us, .. } => {
                ("exception", format!("{code:#04x} {message}"), String::new(), *rtt_us)
            }
            ProbeResult::NoResponse { .. } => ("no_response", String::new(), String::new(), 0),
            ProbeResult::TransportError { message, .. } => {
                ("transport_error", message.clone(), String::new(), 0)
            }
        };
        writeln!(
            w,
            "{};{};{};{};{};{};{};{};{};{:#04x};;{};{};{};{}",
            "probe",
            csv_cell(&rec.key),
            format!("{:?}", rec.target.kind).to_lowercase(),
            csv_cell(&rec.target.endpoint),
            rec.target.port.map(|p| p.to_string()).unwrap_or_default(),
            rec.target.unit_id,
            csv_cell(&vendor),
            csv_cell(&product),
            csv_cell(&revision),
            fc,
            status,
            csv_cell(&value),
            csv_cell(&raw),
            rtt,
        )
        .map_err(|e| e.to_string())?;
    }

    let mut regs: Vec<_> = rec.registers.values().collect();
    regs.sort_by_key(|r| (r.function, r.address));
    for reg in regs {
        writeln!(
            w,
            "{};{};{};{};{};{};{};{};{};{:#04x};{};ok;{};{};{}",
            "register",
            csv_cell(&rec.key),
            format!("{:?}", rec.target.kind).to_lowercase(),
            csv_cell(&rec.target.endpoint),
            rec.target.port.map(|p| p.to_string()).unwrap_or_default(),
            rec.target.unit_id,
            csv_cell(&vendor),
            csv_cell(&product),
            csv_cell(&revision),
            reg.function,
            reg.address,
            reg.raw,
            reg.as_hex,
            reg.rtt_us,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(w.into_bytes())
}

fn device_to_xml(rec: &DeviceRecord) -> Result<Vec<u8>, String> {
    fn escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }
    let (vendor, product, revision) = device_identity(rec);
    let mut w = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<device ");
    w.push_str(&format!(
        "key=\"{}\" transport=\"{}\" endpoint=\"{}\" port=\"{}\" unit_id=\"{}\" vendor=\"{}\" product=\"{}\" revision=\"{}\">\n",
        escape(&rec.key),
        escape(&format!("{:?}", rec.target.kind).to_lowercase()),
        escape(&rec.target.endpoint),
        rec.target.port.map(|p| p.to_string()).unwrap_or_default(),
        rec.target.unit_id,
        escape(&vendor),
        escape(&product),
        escape(&revision),
    ));

    let mut probes: Vec<_> = rec.probes.iter().collect();
    probes.sort_by_key(|(fc, _)| **fc);
    for (fc, probe) in probes {
        let status = match probe {
            ProbeResult::Ok { .. } => "ok",
            ProbeResult::Exception { .. } => "exception",
            ProbeResult::NoResponse { .. } => "no_response",
            ProbeResult::TransportError { .. } => "transport_error",
        };
        w.push_str(&format!(
            "  <probe function=\"{fc:#04x}\" status=\"{status}\">{}</probe>\n",
            escape(&format!("{probe:?}"))
        ));
    }

    let mut regs: Vec<_> = rec.registers.values().collect();
    regs.sort_by_key(|r| (r.function, r.address));
    for reg in regs {
        w.push_str(&format!(
            "  <register function=\"{:#04x}\" address=\"{}\" raw=\"{}\" hex=\"{}\" rtt_us=\"{}\"/>\n",
            reg.function, reg.address, reg.raw, reg.as_hex, reg.rtt_us
        ));
    }
    w.push_str("</device>\n");
    Ok(w.into_bytes())
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

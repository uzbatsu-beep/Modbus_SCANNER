//! Modbus Scanner - backend (Rust/Tauri)
//!
//! Layers:
//! - `modbus`  - protocol implementation (RTU/ASCII framing, TCP/UDP transport, ADU/PDU)
//! - `transport` - async transports: serial (RTU/ASCII), TCP, UDP
//! - `scanner` - scanning engine (strategies, scheduler, results, events)
//! - `commands` - Tauri IPC commands (frontend API boundary)
//! - `state`  - shared application state (Mutex<AppState>)

pub mod commands;
pub mod modbus;
pub mod scanner;
pub mod state;
pub mod transport;

use tauri::Manager;

#[cfg(debug_assertions)]
pub const APP_NAME: &str = "Modbus Scanner (dev)";

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,modbus_scanner_lib=debug".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            use state::AppState;
            app.manage(AppState {
                scanner: scanner::ScannerHandle::new(),
                monitor: scanner::monitor::MonitorHandle::new(),
                config: parking_lot::Mutex::new(state::Config::default()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // system
            commands::system::get_app_info,
            commands::system::get_log_level,
            // serial
            commands::serial::list_serial_ports,
            commands::serial::test_serial_port,
            // scanner
            commands::scanner::start_scan,
            commands::scanner::stop_scan,
            commands::scanner::pause_scan,
            commands::scanner::resume_scan,
            commands::scanner::get_scan_status,
            commands::scanner::get_scan_progress,
            commands::scanner::get_scan_results,
            commands::scanner::clear_results,
            commands::scanner::export_results,
            commands::scanner::export_device,
            commands::scanner::save_project,
            commands::scanner::load_project,
            commands::scanner::start_monitor,
            commands::scanner::stop_monitor,
            commands::scanner::get_monitor_state,
            // modbus
            commands::modbus_cmd::read_registers,
            commands::modbus_cmd::write_registers,
            commands::modbus_cmd::send_raw_frame,
            commands::modbus_cmd::inspect_registers,
            commands::modbus_cmd::decode_register,
            // config
            commands::config::get_config,
            commands::config::save_config,
            commands::config::load_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

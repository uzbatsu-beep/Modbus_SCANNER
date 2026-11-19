//! Tauri IPC commands (frontend API boundary).
//!
//! - `system`     - app info, log level
//! - `serial`     - serial port enumeration, port test
//! - `scanner`    - scan lifecycle: start/stop/pause/resume, results, export
//! - `modbus_cmd` - manual (non-scan) read/write, raw frame send
//! - `config`     - config load/save
//!
//! Error contract: every command returns `Result<T, String>` where the error
//! is a human-readable message (frontend shows it in a toast).

pub mod config;
pub mod modbus_cmd;
pub mod profiles;
pub mod scanner;
pub mod serial;
pub mod system;

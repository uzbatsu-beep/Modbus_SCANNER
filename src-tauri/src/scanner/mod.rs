//! Scanning engine.
//!
//! - `types`      - scan params, targets, results, status (IPC contract types)
//! - `strategies` - scan strategies (unit id range, IP range, register map)
//! - `engine`     - async engine: task management, event emission (Tauri events), pause/resume/stop

pub mod diff;
pub mod engine;
pub mod monitor;
pub mod strategies;
pub mod types;

pub use engine::ScannerHandle;

//! Generate TypeScript IPC bindings from the Rust types into `src/types/generated`.
//! Run with: cargo run --bin gen_bindings

use std::path::Path;

fn main() {
    modbus_scanner_lib::bindings::export_bindings(Path::new(env!("CARGO_MANIFEST_DIR")));
}

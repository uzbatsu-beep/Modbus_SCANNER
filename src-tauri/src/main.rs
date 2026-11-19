// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use modbus_scanner_lib::cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Scan(args)) => {
            // CLI mode: no Tauri runtime, no WebView, plain JSON on stdout.
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "warn".into()),
                )
                .with_writer(std::io::stderr)
                .init();
            let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            if let Err(error) = runtime.block_on(modbus_scanner_lib::cli::run_scan(args)) {
                eprintln!("error: {error}");
                std::process::exit(1);
            }
        }
        Some(Commands::Serve { port }) => {
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "info".into()),
                )
                .with_writer(std::io::stderr)
                .init();
            let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            if let Err(error) = runtime.block_on(modbus_scanner_lib::api::serve_headless(port)) {
                eprintln!("error: {error}");
                std::process::exit(1);
            }
        }
        None => modbus_scanner_lib::run(),
    }
}

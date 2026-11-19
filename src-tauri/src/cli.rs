//! Headless command-line interface (E-02).
//!
//! The same `ScannerHandle` and strategies used by the GUI are driven here with
//! `EventSink::null()` (or a stderr logger), so CLI and GUI cannot diverge.
//! No Tauri runtime is started in CLI mode.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};

use crate::modbus::common::SerialMode;
use crate::scanner::types::{
    IpTransportKind, ScanParams, ScanTarget, SerialPortParams, SerialSweepParams,
};
use crate::scanner::{EventSink, ScannerHandle};
use crate::transport::serial::{DataBitsSerde, ParitySerde, StopBitsSerde};
use crate::transport::tcp::TcpFraming;

#[derive(Debug, Parser)]
#[command(name = "modbus-scanner", version, about = "Modbus RTU/ASCII/TCP/UDP scanner")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Run a scan without the GUI and print/write a JSON report
    Scan(ScanArgs),
    /// Start the local REST + WebSocket API on 127.0.0.1
    Serve {
        /// Listening port (default 8787)
        #[arg(long, default_value_t = 8787)]
        port: u16,
    },
}

#[derive(Debug, clap::Args)]
pub struct ScanArgs {
    /// Serial port name (e.g. COM3 or /dev/ttyUSB0)
    #[arg(long)]
    pub port: Option<String>,
    /// Auto-sweep baud rate and parity before scanning unit IDs
    #[arg(long)]
    pub auto: bool,
    /// Baud rate for a known serial setup (default 9600)
    #[arg(long)]
    pub baud: Option<u32>,
    /// Parity: none, even or odd (default none)
    #[arg(long)]
    pub parity: Option<String>,
    /// Use ASCII framing instead of RTU (serial only)
    #[arg(long)]
    pub ascii: bool,
    /// First unit ID (default 1)
    #[arg(long)]
    pub from: Option<u8>,
    /// Last unit ID (default 247)
    #[arg(long)]
    pub to: Option<u8>,

    /// IP address, range "a-b" or CIDR "192.168.1.0/24"
    #[arg(long)]
    pub ip: Option<String>,
    /// End of an IP range (optional when --ip is a CIDR/single address)
    #[arg(long)]
    pub ip_to: Option<String>,
    /// TCP/UDP port (default 502)
    #[arg(long, default_value_t = 502)]
    pub tcp_port: u16,
    /// Use UDP instead of TCP
    #[arg(long)]
    pub udp: bool,
    /// Use MBAP framing for UDP (default RTU-style ADU)
    #[arg(long)]
    pub udp_mbap: bool,
    /// Use raw RTU over TCP instead of MBAP
    #[arg(long)]
    pub rtu_over_tcp: bool,

    /// Response timeout in ms (default 800)
    #[arg(long)]
    pub timeout_ms: Option<u32>,
    /// Delay between requests in ms (default 20)
    #[arg(long)]
    pub gap_ms: Option<u32>,
    /// Retries before "no response" (default 1)
    #[arg(long)]
    pub retries: Option<u32>,
    /// Max parallel IP probes (default 32)
    #[arg(long)]
    pub concurrency: Option<u32>,
    /// Stop after the first found device
    #[arg(long)]
    pub stop_on_first: bool,
    /// Skip FC43 device identification
    #[arg(long)]
    pub no_identify: bool,

    /// Write the JSON report to this file instead of stdout
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Do not stream events to stderr
    #[arg(long)]
    pub quiet: bool,
    /// Overall scan deadline in seconds (default 3600)
    #[arg(long, default_value_t = 3600)]
    pub max_seconds: u64,
}

fn parity(value: Option<&str>) -> Result<ParitySerde, String> {
    match value.unwrap_or("none").to_lowercase().as_str() {
        "none" | "n" => Ok(ParitySerde::None),
        "even" | "e" => Ok(ParitySerde::Even),
        "odd" | "o" => Ok(ParitySerde::Odd),
        other => Err(format!("unknown parity: {other}")),
    }
}

fn build_target(args: &ScanArgs) -> Result<ScanTarget, String> {
    let from = args.from.unwrap_or(1);
    let to = args.to.unwrap_or(247);
    if from > to {
        return Err("--from must not exceed --to".into());
    }

    if let Some(port) = &args.port {
        let mode = if args.ascii { SerialMode::Ascii } else { SerialMode::Rtu };
        if args.auto {
            let mut sweep = SerialSweepParams::defaults(port.clone(), mode, from, to);
            if let Some(baud) = args.baud {
                sweep.baud_rates = vec![baud];
            }
            return Ok(ScanTarget::SerialAuto { sweep });
        }
        let port_params = SerialPortParams {
            port_name: port.clone(),
            baud_rate: args.baud.unwrap_or(9600),
            data_bits: DataBitsSerde::Eight,
            parity: parity(args.parity.as_deref())?,
            stop_bits: StopBitsSerde::One,
        };
        return Ok(ScanTarget::UnitIdRange {
            from,
            to,
            port: port_params,
            mode,
        });
    }

    if let Some(ip) = &args.ip {
        return Ok(ScanTarget::IpRange {
            from: ip.clone(),
            to: args.ip_to.clone(),
            port: args.tcp_port,
            kind: if args.udp { IpTransportKind::Udp } else { IpTransportKind::Tcp },
        });
    }

    Err("specify --port (serial) or --ip (TCP/UDP)".into())
}

fn build_params(args: &ScanArgs) -> ScanParams {
    let mut params = ScanParams::default();
    if let Some(v) = args.timeout_ms {
        params.timeout_ms = v;
    }
    if let Some(v) = args.gap_ms {
        params.request_gap_ms = v;
    }
    if let Some(v) = args.retries {
        params.retries = v;
    }
    if let Some(v) = args.concurrency {
        params.ip_concurrency = v;
    }
    params.stop_on_first = args.stop_on_first;
    params.identify_devices = !args.no_identify;
    params.udp_mbap = args.udp_mbap;
    params.tcp_framing = if args.rtu_over_tcp {
        TcpFraming::Rtu
    } else {
        TcpFraming::Mbap
    };
    params
}

fn cli_sink(quiet: bool) -> EventSink {
    EventSink::new(
        Arc::new(move |name, payload| {
            if !quiet && name == "scan:event" {
                eprintln!("{payload}");
            }
        }),
        None,
        None,
    )
}

/// Parse and execute a `scan` subcommand.
pub async fn run_scan(args: ScanArgs) -> Result<(), String> {
    let target = build_target(&args)?;
    let params = build_params(&args);
    let scanner = ScannerHandle::new();
    scanner.start(cli_sink(args.quiet), target, params).await?;
    let results = scanner
        .wait_finished(Duration::from_secs(args.max_seconds))
        .await?;

    let report = serde_json::to_vec_pretty(&results).map_err(|e| e.to_string())?;
    match args.output {
        Some(path) => {
            tokio::fs::write(path, report).await.map_err(|e| e.to_string())?;
        }
        None => {
            use std::io::Write;
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(&report).map_err(|e| e.to_string())?;
            stdout.write_all(b"\n").map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_serial_target() {
        let args = ScanArgs {
            port: Some("COM7".into()),
            auto: false,
            baud: Some(19200),
            parity: Some("even".into()),
            ascii: false,
            from: None,
            to: Some(10),
            ip: None,
            ip_to: None,
            tcp_port: 502,
            udp: false,
            udp_mbap: false,
            rtu_over_tcp: false,
            timeout_ms: None,
            gap_ms: None,
            retries: None,
            concurrency: None,
            stop_on_first: false,
            no_identify: false,
            output: None,
            quiet: true,
            max_seconds: 60,
        };
        match build_target(&args).unwrap() {
            ScanTarget::UnitIdRange { from, to, port, mode } => {
                assert_eq!((from, to), (1, 10));
                assert_eq!(port.baud_rate, 19200);
                assert_eq!(port.parity, ParitySerde::Even);
                assert_eq!(mode, SerialMode::Rtu);
            }
            other => panic!("unexpected target: {other:?}"),
        }
    }

    #[test]
    fn builds_ip_and_auto_targets() {
        let mut args = ScanArgs {
            port: None,
            auto: false,
            baud: None,
            parity: None,
            ascii: false,
            from: None,
            to: None,
            ip: Some("192.168.1.0/24".into()),
            ip_to: None,
            tcp_port: 502,
            udp: true,
            udp_mbap: false,
            rtu_over_tcp: false,
            timeout_ms: None,
            gap_ms: None,
            retries: None,
            concurrency: None,
            stop_on_first: false,
            no_identify: false,
            output: None,
            quiet: true,
            max_seconds: 60,
        };
        assert!(matches!(
            build_target(&args).unwrap(),
            ScanTarget::IpRange { kind: IpTransportKind::Udp, .. }
        ));
        args.port = Some("/dev/ttyUSB0".into());
        args.auto = true;
        assert!(matches!(
            build_target(&args).unwrap(),
            ScanTarget::SerialAuto { .. }
        ));
    }
}

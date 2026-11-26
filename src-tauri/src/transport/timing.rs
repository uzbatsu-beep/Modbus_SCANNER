//! Application-level transaction timing; not physical wire measurements.
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::time::Instant;

/// Measured stages; absent stages are null, not fabricated zeros.
#[derive(Debug, Default, Clone, Serialize, Deserialize, TS)]
pub struct TransactionTiming {
    #[ts(type = "number | null")]
    pub connect_us: Option<u64>,
    #[ts(type = "number | null")]
    pub reset_us: Option<u64>,
    #[ts(type = "number | null")]
    pub tx_us: Option<u64>,
    #[ts(type = "number | null")]
    pub rx_us: Option<u64>,
    #[ts(type = "number | null")]
    pub rtt_us: Option<u64>,
    #[ts(type = "number | null")]
    pub validation_us: Option<u64>,
    #[ts(type = "number | null")]
    pub decode_us: Option<u64>,
    #[ts(type = "number | null")]
    pub acquisition_us: Option<u64>,
    pub tx_bytes: u32,
    pub rx_bytes: u32,
    pub tx_complete: bool,
    pub rx_complete: bool,
    pub failure_stage: Option<String>,
}

pub fn micros(start: Instant) -> u64 { start.elapsed().as_micros().min(u64::MAX as u128) as u64 }

//! Canonical observations, bounded history and session statistics.
use std::collections::VecDeque;
use crate::modbus::decode::{decode, DecodedValue};
use super::types::{DataQuality, MonitorSample, MonitorValueState, MonitorWatch};

pub const DEFAULT_HISTORY: u32 = 10_000;
pub const MAX_TOTAL_SAMPLES: usize = 1_000_000;
pub const UI_BATCH_MS: u64 = 200;

#[derive(Clone)]
pub struct LogObservation {
    pub session_id: String,
    pub watch: MonitorWatch,
    pub sample: MonitorSample,
}

pub struct Channel {
    pub state: MonitorValueState,
    history: VecDeque<MonitorSample>,
    pending: Vec<MonitorSample>,
    numeric_count: u64,
}

fn number(value: &DecodedValue) -> Option<f64> {
    match value {
        DecodedValue::Integer { value } => Some(*value as f64),
        DecodedValue::Float { value } if value.is_finite() => Some(*value),
        _ => None,
    }
}

impl Channel {
    pub fn new(session_id: String, watch: MonitorWatch, capacity: u32) -> Self {
        Self {
            state: MonitorValueState {
                session_id, key: watch.key.clone(), watch, latest: None, last_good: None,
                min: None, max: None, avg: None, delta: None, valid_samples: 0,
                invalid_samples: 0, quality: DataQuality::NotSampled, samples: Vec::new(),
                history_capacity: capacity,
            },
            history: VecDeque::new(), pending: Vec::new(), numeric_count: 0,
        }
    }

    pub fn observe(&mut self, timestamp_ms: u64, elapsed_us: u64,
        result: Result<Vec<u16>, (DataQuality, Option<u8>, String)>) -> LogObservation {
        let (raw, value, quality, exception_code, detail) = match result {
            Ok(words) => {
                let decoded = if words.len() != self.state.watch.quantity as usize { None }
                    else if let Some(spec) = &self.state.watch.spec { decode(&words, spec) }
                    else { words.first().map(|v| DecodedValue::Integer { value: i64::from(*v) }) };
                let quality = if decoded.is_some() { DataQuality::Good } else { DataQuality::DecodeError };
                let detail = (quality == DataQuality::DecodeError).then(|| "cannot decode complete observation".to_string());
                (Some(words), decoded, quality, None, detail)
            }
            Err((quality, code, detail)) => (None, None, quality, code, Some(detail)),
        };
        let sequence = self.state.latest.as_ref().map_or(1, |s| s.sequence + 1);
        let sample = MonitorSample { sequence, timestamp_ms, elapsed_us, raw, value, quality, exception_code, detail };
        self.state.quality = quality;
        if quality == DataQuality::Good {
            self.state.valid_samples += 1;
            self.state.delta = None;
            if let Some(n) = sample.value.as_ref().and_then(number) {
                self.state.min = Some(self.state.min.map_or(n, |v| v.min(n)));
                self.state.max = Some(self.state.max.map_or(n, |v| v.max(n)));
                self.numeric_count += 1;
                // Weighted mean avoids overflow for opposite-sign large finite values.
                let k = self.numeric_count as f64;
                let mean = self.state.avg.map_or(n, |m| m * ((k - 1.0) / k) + n / k);
                self.state.avg = mean.is_finite().then_some(mean);
                self.state.delta = self.state.last_good.as_ref().and_then(|s| s.value.as_ref())
                    .and_then(number).map(|last| n - last).filter(|v| v.is_finite());
            }
            self.state.last_good = Some(sample.clone());
        } else {
            self.state.invalid_samples += 1;
            self.state.delta = None;
        }
        self.state.latest = Some(sample.clone());
        if self.history.len() >= self.state.history_capacity as usize { self.history.pop_front(); }
        self.history.push_back(sample.clone());
        self.pending.push(sample.clone());
        LogObservation { session_id: self.state.session_id.clone(), watch: self.state.watch.clone(), sample }
    }

    pub fn snapshot(&self) -> MonitorValueState {
        let mut state = self.state.clone();
        state.samples = self.history.iter().cloned().collect();
        state
    }

    pub fn delta(&mut self) -> Option<MonitorValueState> {
        if self.pending.is_empty() { return None; }
        let mut state = self.state.clone();
        state.samples = std::mem::take(&mut self.pending);
        Some(state)
    }
}

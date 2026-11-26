//! Session diagnostics. Exact nearest-rank percentiles over the last 1000 attempts.
use std::collections::{BTreeMap, VecDeque};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use super::types::{DataQuality, MonitorSample, MonitorWatch};

pub const RTT_WINDOW: usize = 1000;
pub const STALE_INTERVALS: u64 = 3;

/// Session counters plus recent latency and interval statistics.
#[derive(Debug, Default, Clone, Serialize, Deserialize, TS)]
pub struct MetricRow {
    pub key: String,
    pub label: String,
    #[ts(type = "number")]
    pub attempts: u64,
    #[ts(type = "number")]
    pub requests: u64,
    #[ts(type = "number")]
    pub responses: u64,
    #[ts(type = "number")]
    pub reads_ok: u64,
    #[ts(type = "number")]
    pub good_values: u64,
    #[ts(type = "number")]
    pub timeouts: u64,
    #[ts(type = "number")]
    pub exceptions: u64,
    #[ts(type = "number")]
    pub invalid_responses: u64,
    #[ts(type = "number")]
    pub transport_errors: u64,
    #[ts(type = "number")]
    pub decode_errors: u64,
    #[ts(type = "number")]
    pub tx_bytes: u64,
    #[ts(type = "number")]
    pub rx_bytes: u64,
    pub exception_codes: BTreeMap<String, u32>,
    pub failure_stages: BTreeMap<String, u32>,
    pub rtt_count: u32,
    pub rtt_min_us: Option<f64>,
    pub rtt_max_us: Option<f64>,
    pub rtt_avg_us: Option<f64>,
    pub rtt_p50_us: Option<f64>,
    pub rtt_p95_us: Option<f64>,
    pub rtt_p99_us: Option<f64>,
    pub interval_avg_us: Option<f64>,
    pub jitter_avg_us: Option<f64>,
    pub stale: bool,
}

/// Diagnostic snapshot refreshed independently of new observations.
#[derive(Debug, Default, Clone, Serialize, Deserialize, TS)]
pub struct CommunicationSnapshot {
    pub session: MetricRow,
    pub devices: Vec<MetricRow>,
    pub channels: Vec<MetricRow>,
    pub delay_ms: u32,
    #[ts(type = "number | null")]
    pub cycle_us: Option<u64>,
    #[ts(type = "number | null")]
    pub expected_interval_us: Option<u64>,
    #[ts(type = "number")]
    pub elapsed_us: u64,
}

#[derive(Default)]
struct Accumulator {
    row: MetricRow,
    recent: VecDeque<Option<u64>>,
    intervals: VecDeque<u64>,
    jitter: VecDeque<u64>,
    last_observation: Option<u64>,
    last_interval: Option<u64>,
    last_good: bool,
}
fn push<T>(q: &mut VecDeque<T>, v: T) {
    if q.len() >= RTT_WINDOW { q.pop_front(); }
    q.push_back(v);
}
fn mean(q: &VecDeque<u64>) -> Option<f64> {
    (!q.is_empty()).then(|| q.iter().map(|v| *v as f64).sum::<f64>() / q.len() as f64)
}
impl Accumulator {
    fn new(key: String, label: String) -> Self { Self { row: MetricRow { key, label, ..Default::default() }, ..Default::default() } }
    fn add(&mut self, s: &MonitorSample, interval: Option<u64>, jitter: Option<u64>) {
        let r=&mut self.row;
        r.attempts+=1;
        r.requests+=u64::from(s.timing.tx_complete);
        r.responses+=u64::from(s.timing.rx_complete);
        r.tx_bytes+=u64::from(s.timing.tx_bytes);
        r.rx_bytes+=u64::from(s.timing.rx_bytes);
        let valid_response=matches!(s.quality,DataQuality::Good|DataQuality::DecodeError|DataQuality::ModbusException);
        push(&mut self.recent, if valid_response { s.timing.rtt_us } else { None });
        match s.quality {
            DataQuality::Good => {r.reads_ok+=1;r.good_values+=1;}
            DataQuality::DecodeError => {r.reads_ok+=1;r.decode_errors+=1;}
            DataQuality::Timeout => r.timeouts+=1,
            DataQuality::ModbusException => {r.exceptions+=1;if let Some(code)=s.exception_code {*r.exception_codes.entry(format!("0x{code:02X}")).or_default()+=1;}}
            DataQuality::InvalidResponse => r.invalid_responses+=1,
            DataQuality::TransportError => r.transport_errors+=1,
            DataQuality::NotSampled => {}
        }
        if let Some(stage)=&s.timing.failure_stage {*r.failure_stages.entry(stage.clone()).or_default()+=1;}
        if let Some(i)=interval {push(&mut self.intervals,i);}
        if let Some(j)=jitter {push(&mut self.jitter,j);}
        self.last_observation=Some(s.elapsed_us);
        self.last_good=s.quality==DataQuality::Good;
    }
    fn snapshot(&self, now: u64, expected: Option<u64>, running: bool) -> MetricRow {
        let mut r=self.row.clone();
        let mut values: Vec<u64>=self.recent.iter().flatten().copied().collect();
        values.sort_unstable();
        r.rtt_count=values.len() as u32;
        let rank=|percent:usize| -> Option<f64> {
            if values.is_empty(){None}else{Some(values[(values.len()*percent).div_ceil(100)-1] as f64)}
        };
        r.rtt_min_us=values.first().map(|v|*v as f64);
        r.rtt_max_us=values.last().map(|v|*v as f64);
        r.rtt_avg_us=(!values.is_empty()).then(||values.iter().map(|v|*v as f64).sum::<f64>()/values.len() as f64);
        r.rtt_p50_us=rank(50);r.rtt_p95_us=rank(95);r.rtt_p99_us=rank(99);
        r.interval_avg_us=mean(&self.intervals);r.jitter_avg_us=mean(&self.jitter);
        r.stale=running && self.last_good && expected.zip(self.last_observation).is_some_and(|(e,last)|now.saturating_sub(last)>=e.saturating_mul(STALE_INTERVALS));
        r
    }
}

pub struct Metrics {
    session: Accumulator,
    devices: BTreeMap<String,Accumulator>,
    channels: BTreeMap<String,Accumulator>,
    delay_ms:u32,
    cycle_us:Option<u64>,
}
impl Metrics {
    pub fn new(delay_ms:u32)->Self {Self{session:Accumulator::new("session".into(),"Session".into()),devices:BTreeMap::new(),channels:BTreeMap::new(),delay_ms,cycle_us:None}}
    pub fn complete_cycle(&mut self,us:u64){self.cycle_us=Some(us);}
    pub fn observe(&mut self,w:&MonitorWatch,s:&MonitorSample){
        let c=self.channels.entry(w.key.clone()).or_insert_with(||Accumulator::new(w.key.clone(),w.name.clone().unwrap_or_else(||w.key.clone())));
        let interval=c.last_observation.map(|last|s.elapsed_us.saturating_sub(last));
        let jitter=interval.zip(c.last_interval).map(|(i,last)|i.abs_diff(last));
        c.last_interval=interval;
        c.add(s,interval,jitter);
        // Full target, including serial framing and port, is the device identity.
        let key=serde_json::to_string(&w.target).unwrap_or_else(|_|w.key.clone());
        let label=format!("{:?} {}:{} / Unit {}",w.target.kind,w.target.endpoint,w.target.port.map_or_else(||"-".into(),|p|p.to_string()),w.target.unit_id);
        self.devices.entry(key.clone()).or_insert_with(||Accumulator::new(key,label)).add(s,interval,jitter);
        self.session.add(s,interval,jitter);
    }
    pub fn snapshot(&self,now:u64,running:bool)->CommunicationSnapshot {
        let expected=self.cycle_us.map(|us|us.saturating_add(u64::from(self.delay_ms)*1000));
        CommunicationSnapshot{
            session:self.session.snapshot(now,None,running),
            devices:self.devices.values().map(|a|a.snapshot(now,None,running)).collect(),
            channels:self.channels.values().map(|a|a.snapshot(now,expected,running)).collect(),
            delay_ms:self.delay_ms,cycle_us:self.cycle_us,expected_interval_us:expected,elapsed_us:now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::timing::TransactionTiming;
    use crate::modbus::common::TransportKind;
    use crate::scanner::types::DeviceTarget;
    fn watch(key:&str,unit:u8)->MonitorWatch{
        MonitorWatch{key:key.into(),target:DeviceTarget{kind:TransportKind::Tcp,endpoint:"localhost".into(),port:Some(502),unit_id:unit,serial:None,serial_mode:None},function:3,address:0,quantity:1,name:None,unit:None,tag_id:None,spec:None}
    }
    fn sample(t:u64,q:DataQuality,rtt:Option<u64>)->MonitorSample{
        MonitorSample{sequence:t,timestamp_ms:0,elapsed_us:t,timing:TransactionTiming{rtt_us:rtt,tx_complete:true,rx_complete:rtt.is_some(),tx_bytes:12,rx_bytes:if rtt.is_some(){11}else{0},..Default::default()},raw:None,value:None,quality:q,exception_code:(q==DataQuality::ModbusException).then_some(2),detail:None}
    }
    #[test]
    fn window_excludes_timeouts_and_invalid_frames_and_separates_decode(){
        let mut m=Metrics::new(250);
        let a=watch("a",1);
        for i in 1..=1000 {m.observe(&a,&sample(i,DataQuality::Good,Some(i)));}
        m.observe(&a,&sample(1001,DataQuality::Timeout,None));
        m.observe(&a,&sample(1002,DataQuality::InvalidResponse,Some(100_000)));
        m.observe(&a,&sample(1003,DataQuality::DecodeError,Some(1000)));
        m.observe(&a,&sample(1004,DataQuality::ModbusException,Some(1000)));
        let r=m.snapshot(1004,true).session;
        assert_eq!(r.attempts,1004);assert_eq!(r.reads_ok,1001);assert_eq!(r.good_values,1000);
        assert_eq!((r.timeouts,r.invalid_responses,r.decode_errors,r.exceptions),(1,1,1,1));
        assert_eq!(r.rtt_count,998);assert_eq!(r.rtt_min_us,Some(5.0));assert_eq!(r.rtt_max_us,Some(1000.0));
        assert_eq!(r.exception_codes.get("0x02"),Some(&1));
    }
    #[test]
    fn percentile_interval_jitter_and_stale_are_channel_local(){
        let mut m=Metrics::new(250);m.complete_cycle(50_000);
        let a=watch("a",1);let b=watch("b",2);
        for (t,v) in [(100_000,10),(400_000,20),(750_000,30)]{m.observe(&a,&sample(t,DataQuality::Good,Some(v)));}
        m.observe(&b,&sample(800_000,DataQuality::Timeout,None));
        let s=m.snapshot(1_650_000,true);
        assert_eq!(s.devices.len(),2);assert_eq!(s.expected_interval_us,Some(300_000));
        assert_eq!(s.channels[0].rtt_p50_us,Some(20.0));assert_eq!(s.channels[0].rtt_p95_us,Some(30.0));
        assert_eq!(s.channels[0].interval_avg_us,Some(325_000.0));assert_eq!(s.channels[0].jitter_avg_us,Some(50_000.0));
        assert!(s.channels[0].stale);assert!(!s.channels[1].stale);
        assert!(!m.snapshot(1_650_000,false).channels[0].stale);
    }
}

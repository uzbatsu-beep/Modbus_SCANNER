//! Scan strategies: how the target space is enumerated and probed.
//!
//! A strategy produces a stream of [`WorkItem`]s; the engine executes them and
//! feeds results back. Strategies:
//! - [`UnitIdStrategy`]    - serial bus: one transport, unit ids from..to
//! - [`BaudSweepStrategy`] - serial bus with unknown settings: two-phase sweep
//! - [`IpRangeStrategy`]   - network: unit ids, IP range
//! - register plan         - extra reads for a found device

use std::net::Ipv4Addr;

use crate::modbus::common::SerialMode;
use crate::modbus::frame::request;

use super::types::{
    DeviceTarget, ProbeResult, RegisterRange, ScanParams, ScanTarget, SerialFraming,
    SerialSweepParams,
};

/// One unit of scan work.
#[derive(Debug, Clone)]
pub enum WorkItem {
    /// Probe one device (unit id / ip) with one function code.
    Probe {
        target: DeviceTarget,
        function: u8,
        /// Pre-built PDU bytes.
        pdu: Vec<u8>,
    },
    /// Read one register range of an already found device.
    RegisterRead {
        target: DeviceTarget,
        function: u8,
        start: u16,
        quantity: u16,
        pdu: Vec<u8>,
    },
}

/// Probe PDU builder: read N values from address 0 (device presence + access check).
fn probe_pdu(function: u8) -> Option<Vec<u8>> {
    match function {
        0x01 => Some(request::read_coils(0, 1)),
        0x02 => Some(request::read_discrete_inputs(0, 1)),
        0x03 => Some(request::read_holding_registers(0, 1)),
        0x04 => Some(request::read_input_registers(0, 1)),
        _ => None,
    }
}

/// Trait: enumerates the scan space into work items.
pub trait ScanStrategy: Send {
    /// Estimated total work item count (for progress).
    fn total(&self) -> u64;

    /// Poll the next work item. `None` = strategy exhausted.
    fn next_item(&mut self) -> Option<WorkItem>;

    /// Called by the engine when a probe succeeded (device found).
    /// Returns extra work items (e.g. register plan for the found device).
    fn on_device_found(&mut self, _target: &DeviceTarget) -> Vec<WorkItem> {
        Vec::new()
    }

    /// Whether the engine should stop after a device is found (stop_on_first).
    fn stop_on_found(&self) -> bool {
        false
    }
}

/// Serial bus scan: unit ids from..=to on one port.
pub struct UnitIdStrategy {
    port: super::types::SerialPortParams,
    mode: SerialMode,
    unit_ids: Vec<u8>,
    params: ScanParams,
    /// (index over unit_ids, index over probe_functions)
    pos: usize,
    fc_pos: usize,
    done: bool,
}

impl UnitIdStrategy {
    pub fn new(port: super::types::SerialPortParams, mode: SerialMode, from: u8, to: u8, params: ScanParams) -> Self {
        let unit_ids: Vec<u8> = if from <= to {
            (from..=to).collect()
        } else {
            (to..=from).rev().collect()
        };
        let total = (unit_ids.len() as u64) * (params.probe_functions.len() as u64);
        Self {
            port,
            mode,
            unit_ids,
            params,
            pos: 0,
            fc_pos: 0,
            done: total == 0,
        }
    }
}

impl ScanStrategy for UnitIdStrategy {
    fn total(&self) -> u64 {
        (self.unit_ids.len() as u64) * (self.params.probe_functions.len() as u64)
    }

    fn next_item(&mut self) -> Option<WorkItem> {
        if self.done {
            return None;
        }
        loop {
            if self.pos >= self.unit_ids.len() {
                self.done = true;
                return None;
            }
            let unit_id = self.unit_ids[self.pos];
            let function = *self.params.probe_functions.get(self.fc_pos)?;

            self.fc_pos += 1;
            if self.fc_pos >= self.params.probe_functions.len() {
                self.fc_pos = 0;
                self.pos += 1;
            }

            match probe_pdu(function) {
                Some(pdu) => {
                    let target = DeviceTarget {
                        kind: match self.mode {
                            SerialMode::Rtu => crate::modbus::common::TransportKind::Rtu,
                            SerialMode::Ascii => crate::modbus::common::TransportKind::Ascii,
                        },
                        endpoint: self.port.port_name.clone(),
                        port: None,
                        unit_id,
                        serial: Some(self.port.clone()),
                        serial_mode: Some(self.mode),
                    };
                    return Some(WorkItem::Probe {
                        target,
                        function,
                        pdu,
                    });
                }
                None => continue,
            }
        }
    }

    fn on_device_found(&mut self, target: &DeviceTarget) -> Vec<WorkItem> {
        if self.params.register_plan.is_empty() {
            return Vec::new();
        }
        self.params
            .register_plan
            .clone()
            .into_iter()
            .filter_map(|r| register_work(target, &r))
            .collect()
    }

    fn stop_on_found(&self) -> bool {
        self.params.stop_on_first
    }
}

/// Two-phase serial sweep (P2-01).
///
/// Phase 1 probes a handful of likely unit ids across all baud/framing
/// combinations. As soon as one answers with a valid Modbus frame, that
/// setting is locked and phase 2 scans the full unit-id range on it.
/// This avoids waiting a full timeout for every (baud, framing, id) triple.
pub struct BaudSweepStrategy {
    sweep: SerialSweepParams,
    params: ScanParams,
    settings: Vec<(u32, SerialFraming)>,
    full_unit_ids: Vec<u8>,
    /// Phase 1 cursor.
    setting_pos: usize,
    id_pos: usize,
    fc_pos: usize,
    /// Set to the matched setting once phase 1 succeeds.
    locked_setting: Option<usize>,
    /// Phase 2 cursor.
    full_id_pos: usize,
    full_fc_pos: usize,
}

impl BaudSweepStrategy {
    /// Framing-major order: 8N1 over all bauds, then 8E1, then 8O1 — as
    /// recommended by field guides (9600 8N1 → 2400 8N1 → 19200 8N1 → 8E1...).
    pub fn settings_from(sweep: &SerialSweepParams) -> Vec<(u32, SerialFraming)> {
        let mut out = Vec::with_capacity(sweep.framings.len() * sweep.baud_rates.len());
        for framing in &sweep.framings {
            for &baud in &sweep.baud_rates {
                out.push((baud, *framing));
            }
        }
        out
    }

    pub fn new(sweep: SerialSweepParams, params: ScanParams) -> Self {
        let settings = Self::settings_from(&sweep);
        let full_unit_ids = if sweep.unit_from <= sweep.unit_to {
            (sweep.unit_from..=sweep.unit_to).collect()
        } else {
            (sweep.unit_to..=sweep.unit_from).rev().collect()
        };
        Self {
            sweep,
            params,
            settings,
            full_unit_ids,
            setting_pos: 0,
            id_pos: 0,
            fc_pos: 0,
            locked_setting: None,
            full_id_pos: 0,
            full_fc_pos: 0,
        }
    }

    fn target_for(&self, setting: (u32, SerialFraming), unit_id: u8) -> DeviceTarget {
        let (baud, framing) = setting;
        DeviceTarget {
            kind: match self.sweep.mode {
                SerialMode::Rtu => crate::modbus::common::TransportKind::Rtu,
                SerialMode::Ascii => crate::modbus::common::TransportKind::Ascii,
            },
            endpoint: self.sweep.port_name.clone(),
            port: None,
            unit_id,
            serial: Some(super::types::SerialPortParams {
                port_name: self.sweep.port_name.clone(),
                baud_rate: baud,
                data_bits: framing.data_bits,
                parity: framing.parity,
                stop_bits: framing.stop_bits,
            }),
            serial_mode: Some(self.sweep.mode),
        }
    }
}

impl ScanStrategy for BaudSweepStrategy {
    fn total(&self) -> u64 {
        let fcs = self.params.probe_functions.len() as u64;
        (self.settings.len() as u64) * (self.sweep.discovery_unit_ids.len() as u64) * fcs
            + (self.full_unit_ids.len() as u64) * fcs
    }

    fn next_item(&mut self) -> Option<WorkItem> {
        // Phase 2: full unit-id range on the locked setting.
        if let Some(idx) = self.locked_setting {
            loop {
                if self.full_id_pos >= self.full_unit_ids.len() {
                    return None;
                }
                let unit_id = self.full_unit_ids[self.full_id_pos];
                let function = *self.params.probe_functions.get(self.full_fc_pos)?;
                self.full_fc_pos += 1;
                if self.full_fc_pos >= self.params.probe_functions.len() {
                    self.full_fc_pos = 0;
                    self.full_id_pos += 1;
                }
                if let Some(pdu) = probe_pdu(function) {
                    return Some(WorkItem::Probe {
                        target: self.target_for(self.settings[idx], unit_id),
                        function,
                        pdu,
                    });
                }
            }
        }

        // Phase 1: few likely ids across all settings.
        loop {
            if self.setting_pos >= self.settings.len() {
                return None;
            }
            if self.id_pos >= self.sweep.discovery_unit_ids.len() {
                self.setting_pos += 1;
                self.id_pos = 0;
                self.fc_pos = 0;
                continue;
            }
            let unit_id = self.sweep.discovery_unit_ids[self.id_pos];
            let function = *self.params.probe_functions.get(self.fc_pos)?;
            self.fc_pos += 1;
            if self.fc_pos >= self.params.probe_functions.len() {
                self.fc_pos = 0;
                self.id_pos += 1;
            }
            if let Some(pdu) = probe_pdu(function) {
                return Some(WorkItem::Probe {
                    target: self.target_for(self.settings[self.setting_pos], unit_id),
                    function,
                    pdu,
                });
            }
        }
    }

    fn on_device_found(&mut self, target: &DeviceTarget) -> Vec<WorkItem> {
        // Lock the setting that produced the first valid answer.
        if self.locked_setting.is_none() {
            self.locked_setting = Some(self.setting_pos);
        }
        if self.params.register_plan.is_empty() {
            return Vec::new();
        }
        self.params
            .register_plan
            .clone()
            .into_iter()
            .filter_map(|r| register_work(target, &r))
            .collect()
    }

    fn stop_on_found(&self) -> bool {
        self.params.stop_on_first
    }
}

#[cfg(test)]
mod baud_sweep_tests {
    use super::*;

    #[test]
    fn settings_are_framing_major() {
        let mut sweep = SerialSweepParams::defaults("COM1".into(), SerialMode::Rtu, 1, 10);
        sweep.baud_rates = vec![9600, 2400];
        sweep.framings.truncate(2);
        let settings = BaudSweepStrategy::settings_from(&sweep);
        assert_eq!(settings.len(), 4);
        assert_eq!(settings[0].0, 9600);
        assert_eq!(settings[1].0, 2400);
        assert_ne!(settings[1].1.parity, settings[2].1.parity);
    }

    #[test]
    fn locks_setting_after_first_answer() {
        let sweep = SerialSweepParams::defaults("COM1".into(), SerialMode::Rtu, 1, 3);
        let mut strategy = BaudSweepStrategy::new(sweep, ScanParams::default());
        let first = strategy.next_item().unwrap();
        if let WorkItem::Probe { target, .. } = &first {
            strategy.on_device_found(target);
        }
        assert_eq!(strategy.locked_setting, Some(0));
        // Phase 2 starts at the full range, not the next sweep combination.
        let second = strategy.next_item().unwrap();
        if let WorkItem::Probe { target, .. } = second {
            assert_eq!(target.unit_id, 1);
        }
    }
}

/// IP range scan: IPv4 from..=to (or CIDR), one TCP/UDP port, unit ids.
pub struct IpRangeStrategy {
    kind: super::types::IpTransportKind,
    port: u16,
    unit_ids: Vec<u8>,
    ips: Vec<Ipv4Addr>,
    params: ScanParams,
    /// flat index over (ip, unit_id)
    pos: usize,
    fc_pos: usize,
    done: bool,
}

impl IpRangeStrategy {
    pub fn new(
        kind: super::types::IpTransportKind,
        port: u16,
        unit_ids: Vec<u8>,
        ips: Vec<Ipv4Addr>,
        params: ScanParams,
    ) -> Self {
        let total = (ips.len() as u64) * (unit_ids.len() as u64) * (params.probe_functions.len() as u64);
        Self {
            kind,
            port,
            unit_ids,
            ips,
            params,
            pos: 0,
            fc_pos: 0,
            done: total == 0,
        }
    }

    /// Parse a CIDR "192.168.1.0/24" into an address list.
    pub fn parse_cidr(cidr: &str) -> Option<Vec<Ipv4Addr>> {
        let (addr, mask) = cidr.split_once('/')?;
        let mask: u32 = mask.parse().ok()?;
        if mask > 32 {
            return None;
        }
        let base: Ipv4Addr = addr.parse().ok()?;
        let mask_bits = u32::MAX << (32 - mask);
        if mask == 0 {
            return None;
        }
        let start = u32::from(base) & mask_bits;
        let end = start | !mask_bits;
        let list = (start..=end).filter_map(|v| Ipv4Addr::from(v).to_string().parse::<Ipv4Addr>().ok().or(Some(Ipv4Addr::from(v)))).collect();
        Some(list)
    }

    /// Parse "from"-"to" into an address list.
    pub fn parse_range(from: &str, to: &str) -> Option<Vec<Ipv4Addr>> {
        let a: Ipv4Addr = from.parse().ok()?;
        let b: Ipv4Addr = to.parse().ok()?;
        let (a, b) = (u32::from(a), u32::from(b));
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        // Guard: max /16 worth of addresses (65536) to avoid OOM
        if end - start > 65535 {
            return None;
        }
        Some((start..=end).map(Ipv4Addr::from).collect())
    }
}

impl ScanStrategy for IpRangeStrategy {
    fn total(&self) -> u64 {
        (self.ips.len() as u64)
            * (self.unit_ids.len() as u64)
            * (self.params.probe_functions.len() as u64)
    }

    fn next_item(&mut self) -> Option<WorkItem> {
        if self.done {
            return None;
        }
        loop {
            if self.pos >= self.ips.len() * self.unit_ids.len() {
                self.done = true;
                return None;
            }
            let ip = self.ips[self.pos / self.unit_ids.len().max(1)];
            let unit_id = *self.unit_ids.get(self.pos % self.unit_ids.len().max(1))?;
            let function = *self.params.probe_functions.get(self.fc_pos)?;

            self.fc_pos += 1;
            if self.fc_pos >= self.params.probe_functions.len() {
                self.fc_pos = 0;
                self.pos += 1;
            }

            match probe_pdu(function) {
                Some(pdu) => {
                    let target = DeviceTarget {
                        kind: match self.kind {
                            super::types::IpTransportKind::Tcp => {
                                crate::modbus::common::TransportKind::Tcp
                            }
                            super::types::IpTransportKind::Udp => {
                                crate::modbus::common::TransportKind::Udp
                            }
                        },
                        endpoint: ip.to_string(),
                        port: Some(self.port),
                        unit_id,
                        serial: None,
                        serial_mode: None,
                    };
                    return Some(WorkItem::Probe {
                        target,
                        function,
                        pdu,
                    });
                }
                None => continue,
            }
        }
    }

    fn on_device_found(&mut self, target: &DeviceTarget) -> Vec<WorkItem> {
        if self.params.register_plan.is_empty() {
            return Vec::new();
        }
        self.params
            .register_plan
            .clone()
            .into_iter()
            .filter_map(|r| register_work(target, &r))
            .collect()
    }

    fn stop_on_found(&self) -> bool {
        self.params.stop_on_first
    }
}

fn register_work(target: &DeviceTarget, r: &RegisterRange) -> Option<WorkItem> {
    let pdu = match r.function {
        0x01 => request::read_coils(r.start, r.quantity),
        0x02 => request::read_discrete_inputs(r.start, r.quantity),
        0x03 => request::read_holding_registers(r.start, r.quantity),
        0x04 => request::read_input_registers(r.start, r.quantity),
        _ => return None,
    };
    Some(WorkItem::RegisterRead {
        target: target.clone(),
        function: r.function,
        start: r.start,
        quantity: r.quantity,
        pdu,
    })
}

/// Build a strategy from the IPC scan target.
pub fn build_strategy(target: ScanTarget, params: ScanParams) -> Result<Box<dyn ScanStrategy + Send>, String> {
    match target {
        ScanTarget::UnitIdRange {
            from,
            to,
            port,
            mode,
        } => Ok(Box::new(UnitIdStrategy::new(port, mode, from, to, params))),
        ScanTarget::SerialAuto { sweep } => {
            if sweep.baud_rates.is_empty() || sweep.framings.is_empty() || sweep.discovery_unit_ids.is_empty() {
                return Err("baud rates, framings and discovery unit ids must not be empty".into());
            }
            Ok(Box::new(BaudSweepStrategy::new(sweep, params)))
        }
        ScanTarget::IpRange {
            from,
            to,
            port,
            kind,
        } => {
            let ips = match to {
                Some(t) => IpRangeStrategy::parse_range(&from, &t),
                None => match from.split_once('/') {
                    Some(_) => IpRangeStrategy::parse_cidr(&from),
                    None => {
                        let a: Ipv4Addr = from.parse().map_err(|e| format!("bad address {from}: {e}"))?;
                        Some(vec![a])
                    }
                },
            }
            .ok_or_else(|| "invalid IP range (max 65536 addresses)".to_string())?;
            if ips.is_empty() {
                return Err("empty IP range".into());
            }
            // IP scan: probe unit id 1 (and 255) by default — most TCP devices
            let unit_ids = if params.probe_functions.is_empty() {
                vec![1]
            } else {
                vec![1, 255]
            };
            Ok(Box::new(IpRangeStrategy::new(kind, port, unit_ids, ips, params)))
        }
        ScanTarget::SingleDevice {
            target,
            register_plan,
        } => {
            let mut params = params;
            params.register_plan = register_plan;
            let kind = match target.kind {
                crate::modbus::common::TransportKind::Rtu => SerialMode::Rtu,
                _ => SerialMode::Rtu,
            };
            let port = target.serial.clone().ok_or("serial params required")?;
            let mut s = UnitIdStrategy::new(port, kind, target.unit_id, target.unit_id, params.clone());
            // Note: SingleDevice uses probe_functions + register_plan
            let _ = &mut s;
            Ok(Box::new(s))
        }
    }
}

/// Parse a probe result into a `ProbeResult` (ok / exception).
pub fn classify_response(function: u8, pdu: &[u8], rtt_us: u128) -> ProbeResult {
    use crate::modbus::frame::Pdu;
    match Pdu::from_bytes(pdu) {
        Ok(p) if p.is_exception() => {
            if let Pdu::Exception { code, .. } = p {
                ProbeResult::Exception {
                    function,
                    code,
                    message: crate::modbus::common::ModbusError::from_exception(code)
                        .to_string(),
                    rtt_us,
                }
            } else {
                unreachable!()
            }
        }
        Ok(p) => {
            let data = match &p {
                Pdu::Response { data, .. } => data.clone(),
                _ => Vec::new(),
            };
            let values = match function {
                0x01 | 0x02 => crate::modbus::frame::response::parse_bits(&data, 1)
                    .map(|bits| bits.iter().map(|&b| b as u16).collect())
                    .unwrap_or_default(),
                0x03 | 0x04 => crate::modbus::frame::response::parse_registers(&data).unwrap_or_default(),
                _ => Vec::new(),
            };
            ProbeResult::Ok {
                function,
                values,
                raw_hex: hex::encode(pdu),
                rtt_us,
            }
        }
        Err(e) => ProbeResult::TransportError {
            function,
            message: e.to_string(),
        },
    }
}

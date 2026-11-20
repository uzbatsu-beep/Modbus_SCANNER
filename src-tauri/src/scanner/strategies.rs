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
    DeviceTarget, ProbeResult, RegisterRange, RegisterScanMode, RegisterScanParams, ScanParams,
    ScanTarget, SerialFraming, SerialSweepParams,
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

/// Outcome of one register/coil block read, fed back to the strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterReadOutcome {
    /// Block answered with data.
    Ok { values: Vec<u16> },
    /// Modbus exception (0x01/0x03 abort the whole scan).
    Exception { code: u8 },
    /// No answer after retries — never split, counted toward the abort limit.
    Timeout,
    /// Transport-level failure.
    Error,
    /// Received invalid PDU/count/FC. Abort adaptive register discovery so a
    /// broken device cannot trigger thousands of follow-up split requests.
    Malformed,
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

    /// Called after a register read completes. Adaptive strategies use this to
    /// schedule follow-up blocks; others return nothing.
    fn on_register_read(&mut self, _item: &WorkItem, _outcome: RegisterReadOutcome) -> Vec<WorkItem> {
        Vec::new()
    }

    /// Whether the engine should stop after a device is found (stop_on_first).
    fn stop_on_found(&self) -> bool {
        false
    }
}

/// A single existing device over any supported transport (serial or IP).
pub struct SingleDeviceStrategy {
    target: DeviceTarget,
    params: ScanParams,
    fc_pos: usize,
}

impl ScanStrategy for SingleDeviceStrategy {
    fn total(&self) -> u64 {
        self.params.probe_functions.len() as u64
    }

    fn next_item(&mut self) -> Option<WorkItem> {
        while let Some(&function) = self.params.probe_functions.get(self.fc_pos) {
            self.fc_pos += 1;
            if let Some(pdu) = probe_pdu(function) {
                return Some(WorkItem::Probe {
                    target: self.target.clone(),
                    function,
                    pdu,
                });
            }
        }
        None
    }

    fn on_device_found(&mut self, target: &DeviceTarget) -> Vec<WorkItem> {
        self.params.register_plan.iter().filter_map(|r| register_work(target, r)).collect()
    }

    fn stop_on_found(&self) -> bool {
        self.params.stop_on_first
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Block {
    start: u16,
    len: u16,
}

/// Adaptive register-map scan (P2-03).
///
/// A readable block is accepted as a whole. A block that answers with exception
/// 02 is split into fifths (Quick/Balanced/Deep differ in how far they recurse),
/// then edges are refined register by register. Timeouts are never split and
/// three consecutive timeouts abort the scan, as does exception 01 (unsupported
/// function) or 03 (illegal value).
pub struct RegisterScanStrategy {
    spec: RegisterScanParams,
    queue: std::collections::VecDeque<Block>,
    singles_queued: std::collections::HashSet<u16>,
    readable: std::collections::HashSet<u16>,
    timeouts_in_row: u32,
    aborted: bool,
}

impl RegisterScanStrategy {
    fn batch_for(spec: &RegisterScanParams) -> u16 {
        let max_qty = match crate::modbus::frame::FunctionCode::from_u8(spec.function) {
            Some(fc) => fc.max_quantity().0,
            None => 125,
        };
        let default = match spec.mode {
            RegisterScanMode::Quick | RegisterScanMode::Balanced => 100,
            RegisterScanMode::Deep => 20,
        };
        let batch = if spec.batch == 0 { default } else { spec.batch };
        batch.clamp(1, max_qty)
    }

    fn enqueue_single(&mut self, addr: u16) {
        if !self.readable.contains(&addr) && self.singles_queued.insert(addr) {
            self.queue.push_back(Block { start: addr, len: 1 });
        }
    }

    fn enqueue_singles(&mut self, block: Block) {
        for addr in block.start..=block.start.saturating_add(block.len - 1) {
            self.enqueue_single(addr);
        }
    }

    fn split_into(&mut self, block: Block, parts: u16) {
        if block.len <= 1 {
            return;
        }
        let parts = parts.min(block.len);
        let base = block.len / parts;
        let rem = block.len % parts;
        // The end-exclusive cursor may reach 65536 at the last register.
        let mut cursor = u32::from(block.start);
        for i in 0..parts {
            let len = base + if i < rem { 1 } else { 0 };
            if len == 0 {
                continue;
            }
            self.queue.push_back(Block { start: cursor as u16, len });
            cursor += u32::from(len);
        }
    }

    /// Split a failed block according to the selected strategy.
    fn split_failed(&mut self, block: Block) {
        match self.spec.mode {
            // Quick: one split into fifths, then refine edges to single registers.
            RegisterScanMode::Quick => {
                if block.len > 5 {
                    self.split_into(block, 5);
                } else {
                    self.enqueue_singles(block);
                }
            }
            // Balanced: 100 → 20 → 5 → single registers.
            RegisterScanMode::Balanced => {
                if block.len > 20 {
                    self.split_into(block, 5);
                } else if block.len > 5 {
                    self.split_into(block, 5);
                } else {
                    self.enqueue_singles(block);
                }
            }
            // Deep: keep splitting until single registers.
            RegisterScanMode::Deep => {
                if block.len > 1 {
                    self.split_into(block, 5);
                } else {
                    self.enqueue_single(block.start);
                }
            }
        }
    }

    pub fn new(spec: RegisterScanParams, _params: ScanParams) -> Self {
        let batch = Self::batch_for(&spec);
        let mut queue = std::collections::VecDeque::new();
        // u32 arithmetic permits the full 0..=65535 address space.
        let mut cursor = u32::from(spec.start);
        let end = u32::from(spec.end);
        while cursor <= end {
            let len = u32::from(batch).min(end - cursor + 1) as u16;
            queue.push_back(Block { start: cursor as u16, len });
            cursor += u32::from(len);
        }
        Self {
            spec,
            queue,
            singles_queued: std::collections::HashSet::new(),
            readable: std::collections::HashSet::new(),
            timeouts_in_row: 0,
            aborted: false,
        }
    }
}

impl ScanStrategy for RegisterScanStrategy {
    fn total(&self) -> u64 {
        // Initial blocks plus the worst-case edge refinement.
        (self.spec.end.saturating_sub(self.spec.start) as u64) + 2
    }

    fn next_item(&mut self) -> Option<WorkItem> {
        if self.aborted {
            return None;
        }
        let block = self.queue.pop_front()?;
        let range = RegisterRange {
            function: self.spec.function,
            start: block.start,
            quantity: block.len,
        };
        register_work(&self.spec.target, &range)
    }

    fn on_register_read(&mut self, item: &WorkItem, outcome: RegisterReadOutcome) -> Vec<WorkItem> {
        let (start, quantity) = match item {
            WorkItem::RegisterRead { start, quantity, .. } => (*start, *quantity),
            _ => return Vec::new(),
        };
        let block = Block { start, len: quantity };

        match outcome {
            RegisterReadOutcome::Ok { values } => {
                self.timeouts_in_row = 0;
                let answered = values.len().min(quantity as usize);
                for i in 0..answered {
                    self.readable.insert(start + i as u16);
                }
                // Rare: device returned fewer values than requested.
                if answered < quantity as usize {
                    let first_missing = start + answered as u16;
                    let len = quantity - answered as u16;
                    self.queue.push_back(Block { start: first_missing, len });
                }
            }
            RegisterReadOutcome::Exception { code } => {
                self.timeouts_in_row = 0;
                if code == 0x01 || code == 0x03 {
                    // Function unsupported / illegal value: nothing more to scan.
                    self.aborted = true;
                    self.queue.clear();
                } else {
                    self.split_failed(block);
                }
            }
            RegisterReadOutcome::Timeout => {
                self.timeouts_in_row += 1;
                if self.timeouts_in_row >= 3 {
                    self.aborted = true;
                    self.queue.clear();
                }
                // Timeouts are never split.
            }
            RegisterReadOutcome::Error => {
                self.timeouts_in_row = 0;
                self.split_failed(block);
            }
            RegisterReadOutcome::Malformed => {
                self.aborted = true;
                self.queue.clear();
            }
        }
        Vec::new()
    }

    fn stop_on_found(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod register_scan_tests {
    use super::*;

    fn spec(mode: RegisterScanMode, batch: u16) -> RegisterScanParams {
        RegisterScanParams {
            target: DeviceTarget {
                kind: crate::modbus::common::TransportKind::Tcp,
                endpoint: "127.0.0.1".into(),
                port: Some(502),
                unit_id: 1,
                serial: None,
                serial_mode: None,
            },
            function: 0x03,
            start: 0,
            end: 99,
            mode,
            batch,
        }
    }

    #[test]
    fn covers_range_with_initial_blocks() {
        let mut s = RegisterScanStrategy::new(spec(RegisterScanMode::Balanced, 100), ScanParams::default());
        let item = s.next_item().unwrap();
        if let WorkItem::RegisterRead { start, quantity, .. } = item {
            assert_eq!(start, 0);
            assert_eq!(quantity, 100);
        }
        assert!(s.next_item().is_none());
    }

    #[test]
    fn failed_block_is_split_into_fifths_then_refined() {
        let mut s = RegisterScanStrategy::new(spec(RegisterScanMode::Balanced, 100), ScanParams::default());
        let item = s.next_item().unwrap();
        s.on_register_read(&item, RegisterReadOutcome::Exception { code: 0x02 });
        assert_eq!(s.queue.len(), 5);
        // Each failed fifth of 20 splits again into five blocks of 4.
        let sub = s.queue.pop_front().unwrap();
        s.on_register_read(
            &WorkItem::RegisterRead {
                target: s.spec.target.clone(),
                function: 0x03,
                start: sub.start,
                quantity: sub.len,
                pdu: Vec::new(),
            },
            RegisterReadOutcome::Exception { code: 0x02 },
        );
        assert_eq!(s.queue.len(), 9); // 4 remaining fifths + 5 children
    }

    #[test]
    fn aborts_on_illegal_function() {
        let mut s = RegisterScanStrategy::new(spec(RegisterScanMode::Deep, 20), ScanParams::default());
        let item = s.next_item().unwrap();
        s.on_register_read(&item, RegisterReadOutcome::Exception { code: 0x01 });
        assert!(s.aborted);
        assert!(s.next_item().is_none());
    }

    #[test]
    fn three_timeouts_abort() {
        let mut s = RegisterScanStrategy::new(spec(RegisterScanMode::Quick, 20), ScanParams::default());
        for _ in 0..3 {
            let item = s.next_item().unwrap();
            s.on_register_read(&item, RegisterReadOutcome::Timeout);
        }
        assert!(s.aborted);
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
        // A CIDR must never allocate an unbounded address vector (e.g. /8).
        // At most /16 is allowed, matching parse_range()'s 65_536 cap.
        if mask < 16 {
            return None;
        }
        let base: Ipv4Addr = addr.parse().ok()?;
        let mask_bits = u32::MAX << (32 - mask);
        let start = u32::from(base) & mask_bits;
        let end = start | !mask_bits;
        Some((start..=end).map(Ipv4Addr::from).collect())
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
    let max_qty = match r.function {
        0x01 | 0x02 => 2_000u16,
        0x03 | 0x04 => 125u16,
        _ => return None,
    };
    if r.quantity == 0 || r.quantity > max_qty ||
        (r.start as u32 + r.quantity as u32) > 65_536 {
        return None;
    }
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
    // Field safety invariant: discovery may issue only FC01..FC04. Never silently
    // skip unsupported codes or accept an empty probe plan. Adaptive register
    // scanning has an independently validated read-only FC.
    if !matches!(&target, ScanTarget::RegisterScan { .. }) {
        if params.probe_functions.is_empty() ||
            params.probe_functions.iter().any(|fc| !matches!(fc, 0x01..=0x04)) {
            return Err("discovery requires a non-empty read-only probe list (FC01..FC04)".into());
        }
    }
    match target {
        ScanTarget::UnitIdRange {
            from,
            to,
            port,
            mode,
        } => {
            if !(1..=247).contains(&from) || !(1..=247).contains(&to) {
                return Err("serial unit IDs must be 1..=247 (0 is broadcast)".into());
            }
            Ok(Box::new(UnitIdStrategy::new(port, mode, from, to, params)))
        },
        ScanTarget::SerialAuto { sweep } => {
            if sweep.baud_rates.is_empty() || sweep.framings.is_empty() || sweep.discovery_unit_ids.is_empty() {
                return Err("baud rates, framings and discovery unit ids must not be empty".into());
            }
            if sweep.discovery_unit_ids.iter().any(|&id| !(1..=247).contains(&id))
                || !(1..=247).contains(&sweep.unit_from) || !(1..=247).contains(&sweep.unit_to) {
                return Err("serial unit IDs must be 1..=247 (0 is broadcast)".into());
            }
            Ok(Box::new(BaudSweepStrategy::new(sweep, params)))
        }
        ScanTarget::RegisterScan { scan } => {
            if scan.start > scan.end {
                return Err("register scan start must not exceed end".into());
            }
            if !matches!(scan.function, 0x01..=0x04) {
                return Err(format!("register scan only supports read FC01..FC04 (got {:#04x})", scan.function));
            }
            Ok(Box::new(RegisterScanStrategy::new(scan, params)))
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
        ScanTarget::SingleDevice { target, register_plan } => {
            if matches!(target.kind, crate::modbus::common::TransportKind::Rtu |
                crate::modbus::common::TransportKind::Ascii) && target.serial.is_none() {
                return Err("serial params required for RTU/ASCII".into());
            }
            let mut params = params;
            params.register_plan = register_plan;
            Ok(Box::new(SingleDeviceStrategy { target, params, fc_pos: 0 }))
        }
    }
}

/// Strictly classify one discovery response. A malformed FC/count/quantity
/// is an error, not a discovered device with empty register values.
pub fn classify_response(function: u8, pdu: &[u8], rtt_us: u128) -> ProbeResult {
    use crate::modbus::frame::{response, validate_response_function, Pdu};

    let decoded = Pdu::from_bytes(pdu)
        .and_then(|p| validate_response_function(p, function))
        .and_then(|p| match p {
            Pdu::Exception { code, .. } => Ok(Err(code)),
            Pdu::Response { data, .. } => {
                response::parse_read_values(function, &data, 1).map(Ok)
            }
            _ => Err(crate::modbus::common::ModbusError::InvalidFrame(
                "expected a response PDU".into()
            )),
        });
    match decoded {
        Ok(Ok(values)) => ProbeResult::Ok {
            function,
            values,
            raw_hex: hex::encode(pdu),
            rtt_us,
        },
        Ok(Err(code)) => ProbeResult::Exception {
            function,
            code,
            message: crate::modbus::common::ModbusError::from_exception(code).to_string(),
            rtt_us,
        },
        Err(e) => ProbeResult::TransportError {
            function,
            message: e.to_string(),
        },
    }
}


#[cfg(test)]
mod reliability_tests {
    use super::*;

    #[test]
    fn full_register_address_space_does_not_overflow() {
        let target = DeviceTarget {
            kind: crate::modbus::common::TransportKind::Tcp,
            endpoint: "127.0.0.1".into(), port: Some(502),
            unit_id: 1, serial: None, serial_mode: None,
        };
        let scan = RegisterScanParams {
            target, function: 3, start: 0, end: u16::MAX,
            mode: RegisterScanMode::Balanced, batch: 100,
        };
        let mut s = RegisterScanStrategy::new(scan, ScanParams::default());
        let mut last = 0u32;
        let mut count = 0;
        while let Some(WorkItem::RegisterRead { start, quantity, .. }) = s.next_item() {
            last = start as u32 + quantity as u32;
            count += 1;
        }
        assert_eq!(last, 65_536);
        assert!(count > 600);
    }

    #[test]
    fn rejects_oversized_cidr_before_allocation() {
        assert!(IpRangeStrategy::parse_cidr("10.0.0.0/0").is_none());
        assert!(IpRangeStrategy::parse_cidr("10.0.0.0/8").is_none());
        assert!(IpRangeStrategy::parse_cidr("10.0.0.0/15").is_none());
        assert_eq!(IpRangeStrategy::parse_cidr("10.20.30.0/24").unwrap().len(), 256);
        assert_eq!(IpRangeStrategy::parse_cidr("10.20.30.9/32").unwrap().len(), 1);
    }

    #[test]
    fn single_device_tcp_needs_no_serial_configuration() {
        let target = DeviceTarget {
            kind: crate::modbus::common::TransportKind::Tcp,
            endpoint: "192.168.0.10".into(),
            port: Some(502),
            unit_id: 1,
            serial: None,
            serial_mode: None,
        };
        let mut strategy = build_strategy(
            ScanTarget::SingleDevice { target, register_plan: Vec::new() },
            ScanParams::default(),
        ).unwrap();
        assert!(matches!(strategy.next_item(), Some(WorkItem::Probe { .. })));
    }

    #[test]
    fn register_scan_rejects_write_functions_and_broadcast_discovery() {
        let target = DeviceTarget {
            kind: crate::modbus::common::TransportKind::Tcp,
            endpoint: "127.0.0.1".into(),
            port: Some(502),
            unit_id: 1,
            serial: None,
            serial_mode: None,
        };
        let scan = RegisterScanParams {
            target, function: 0x10, start: 0, end: 10,
            mode: RegisterScanMode::Balanced, batch: 0,
        };
        assert!(build_strategy(ScanTarget::RegisterScan { scan }, ScanParams::default()).is_err());
        let port = super::super::types::SerialPortParams {
            port_name: "COM3".into(), baud_rate: 9600,
            data_bits: crate::transport::serial::DataBitsSerde::Eight,
            parity: crate::transport::serial::ParitySerde::None,
            stop_bits: crate::transport::serial::StopBitsSerde::One,
        };
        assert!(build_strategy(
            ScanTarget::UnitIdRange { from: 0, to: 1, port, mode: SerialMode::Rtu },
            ScanParams::default(),
        ).is_err());
    }
}

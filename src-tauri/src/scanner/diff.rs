//! Compare two scan snapshots (P3-08).

use std::collections::BTreeMap;

use super::types::{
    ChangeKind, DeviceDiff, RegisterDiff, ScanDiff, ScanResults,
};

/// Compare register maps of two scans. Only common devices are diffed at the
/// register level; devices present on one side are listed separately.
pub fn compare(
    left: &ScanResults,
    right: &ScanResults,
    left_label: String,
    right_label: String,
) -> ScanDiff {
    let left_devices: BTreeMap<_, _> = left
        .records
        .iter()
        .map(|r| (r.key.as_str(), r))
        .collect();
    let right_devices: BTreeMap<_, _> = right
        .records
        .iter()
        .map(|r| (r.key.as_str(), r))
        .collect();

    let added_devices: Vec<String> = right_devices
        .keys()
        .filter(|k| !left_devices.contains_key(**k))
        .map(|k| k.to_string())
        .collect();
    let removed_devices: Vec<String> = left_devices
        .keys()
        .filter(|k| !right_devices.contains_key(**k))
        .map(|k| k.to_string())
        .collect();

    let mut devices = Vec::new();
    let mut registers = Vec::new();
    let mut added = 0u32;
    let mut removed = 0u32;
    let mut changed = 0u32;

    for (key, right_rec) in &right_devices {
        let Some(left_rec) = left_devices.get(key) else {
            continue;
        };
        let mut counters = DeviceDiff {
            device_key: key.to_string(),
            added: 0,
            removed: 0,
            changed: 0,
        };

        let left_regs: BTreeMap<_, _> = left_rec
            .registers
            .values()
            .map(|v| ((v.function, v.address), v))
            .collect();
        let right_regs: BTreeMap<_, _> = right_rec
            .registers
            .values()
            .map(|v| ((v.function, v.address), v))
            .collect();

        let mut all_keys: Vec<_> = left_regs.keys().chain(right_regs.keys()).copied().collect();
        all_keys.sort_unstable();
        all_keys.dedup();

        for addr_key in all_keys {
            let (function, address) = addr_key;
            let old_value = left_regs.get(&addr_key).map(|v| v.raw);
            let new_value = right_regs.get(&addr_key).map(|v| v.raw);
            let change = match (old_value, new_value) {
                (None, Some(_)) => {
                    added += 1;
                    counters.added += 1;
                    ChangeKind::Added
                }
                (Some(_), None) => {
                    removed += 1;
                    counters.removed += 1;
                    ChangeKind::Removed
                }
                (Some(a), Some(b)) if a != b => {
                    changed += 1;
                    counters.changed += 1;
                    ChangeKind::Changed
                }
                _ => ChangeKind::Unchanged,
            };
            if change != ChangeKind::Unchanged {
                registers.push(RegisterDiff {
                    device_key: key.to_string(),
                    function,
                    address,
                    change,
                    old_value,
                    new_value,
                });
            }
        }
        if counters.added > 0 || counters.removed > 0 || counters.changed > 0 {
            devices.push(counters);
        }
    }

    ScanDiff {
        left_label,
        right_label,
        left_devices: left.records.len(),
        right_devices: right.records.len(),
        added_devices,
        removed_devices,
        devices,
        registers,
        added,
        removed,
        changed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::types::{DeviceRecord, RegisterValue, ScanResults};
    use std::collections::HashMap;

    fn scan(value: u16) -> ScanResults {
        let mut registers = HashMap::new();
        registers.insert(
            "03:0000".to_string(),
            RegisterValue {
                function: 3,
                address: 0,
                raw: value,
                as_hex: format!("{value:04x}"),
                rtt_us: 0,
            },
        );
        let record = DeviceRecord {
            key: "tcp:1.2.3.4:502:1".into(),
            target: crate::scanner::types::DeviceTarget {
                kind: crate::modbus::common::TransportKind::Tcp,
                endpoint: "1.2.3.4".into(),
                port: Some(502),
                unit_id: 1,
                serial: None,
                serial_mode: None,
            },
            probes: HashMap::new(),
            registers,
            device_identification: None,
            first_seen_ms: 0,
            last_seen_ms: 0,
        };
        ScanResults {
            records: vec![record],
            ip_targets: Vec::new(),
            stats: Default::default(),
        }
    }

    #[test]
    fn detects_changed_and_added() {
        let mut right = scan(20);
        right.records[0].registers.insert(
            "03:0001".to_string(),
            RegisterValue {
                function: 3,
                address: 1,
                raw: 99,
                as_hex: "0063".into(),
                rtt_us: 0,
            },
        );
        let diff = compare(&scan(10), &right, "before".into(), "after".into());
        assert_eq!(diff.changed, 1);
        assert_eq!(diff.added, 1);
        assert_eq!(diff.registers.len(), 2);
        assert!(diff.registers.iter().any(|r| r.change == ChangeKind::Changed && r.old_value == Some(10)));
        assert!(diff.registers.iter().any(|r| r.change == ChangeKind::Added && r.address == 1));
    }
}

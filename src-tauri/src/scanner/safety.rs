//! Industrial OT IP guard. Empty allowlist denies all network scans.
use std::net::{IpAddr, Ipv4Addr};
use super::types::{DeviceTarget, ScanParams};
use crate::modbus::common::TransportKind;

#[derive(Debug, Clone, Copy)]
struct Network { ip: IpAddr, prefix: u8 }
impl Network {
    fn parse(raw: &str) -> Result<Self, String> {
        let (addr, bits) = match raw.trim().split_once('/') {
            Some((a, b)) => (a, Some(b)), None => (raw.trim(), None),
        };
        let ip: IpAddr = addr.parse().map_err(|_| format!("invalid OT IP/CIDR: {raw}"))?;
        let max = if ip.is_ipv4() { 32 } else { 128 };
        let prefix = match bits {
            Some(s) => s.parse::<u8>().map_err(|_| format!("bad CIDR: {raw}"))?,
            None => max,
        };
        if prefix > max { return Err(format!("invalid CIDR prefix: {raw}")); }
        Ok(Self { ip, prefix })
    }
    fn contains(self, other: IpAddr) -> bool {
        match (self.ip, other) {
            (IpAddr::V4(a), IpAddr::V4(b)) => {
                let mask = if self.prefix == 0 { 0 } else { u32::MAX << (32 - self.prefix) };
                u32::from(a) & mask == u32::from(b) & mask
            },
            (IpAddr::V6(a), IpAddr::V6(b)) => {
                let mask = if self.prefix == 0 { 0 } else { u128::MAX << (128 - self.prefix) };
                u128::from(a) & mask == u128::from(b) & mask
            },
            _ => false,
        }
    }
}

pub struct IpPolicy { allowed: Vec<Network>, denied: Vec<Network> }
impl IpPolicy {
    pub fn new(p: &ScanParams) -> Result<Self, String> {
        Ok(Self {
            allowed: p.ip_allowlist.iter().map(|s| Network::parse(s)).collect::<Result<Vec<_>, _>>()?,
            denied: p.ip_exclusions.iter().map(|s| Network::parse(s)).collect::<Result<Vec<_>, _>>()?,
        })
    }
    pub fn check_ip(&self, ip: IpAddr) -> Result<bool, String> {
        if self.allowed.is_empty() {
            return Err("OT policy: empty IP allowlist; explicitly authorize the destination IP/CIDR".into());
        }
        if self.denied.iter().any(|net| net.contains(ip)) { return Ok(false); }
        if !self.allowed.iter().any(|net| net.contains(ip)) {
            return Err(format!("OT policy: {ip} outside authorized IP allowlist"));
        }
        Ok(true)
    }
    pub fn check_device(&self, target: &DeviceTarget) -> Result<(), String> {
        if !matches!(target.kind, TransportKind::Tcp | TransportKind::Udp) { return Ok(()); }
        let ip: IpAddr = target.endpoint.parse().map_err(|_| {
            "OT policy: IP scans require a literal IP, not a DNS hostname".to_string()
        })?;
        if !self.check_ip(ip)? { return Err(format!("OT policy: {ip} is explicitly excluded")); }
        Ok(())
    }
    pub fn filter_range(&self, ips: Vec<Ipv4Addr>) -> Result<Vec<Ipv4Addr>, String> {
        let mut filtered = Vec::with_capacity(ips.len());
        for ip in ips { if self.check_ip(IpAddr::V4(ip))? { filtered.push(ip); } }
        if filtered.is_empty() {
            return Err("OT policy: no authorized, non-excluded IP targets remain".into());
        }
        Ok(filtered)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn params(allow: &[&str], exclude: &[&str]) -> ScanParams {
        ScanParams {
            ip_allowlist: allow.iter().map(|s| s.to_string()).collect(),
            ip_exclusions: exclude.iter().map(|s| s.to_string()).collect(),
            ..ScanParams::default()
        }
    }
    #[test]
    fn default_deny_and_cidr_validation() {
        assert!(IpPolicy::new(&ScanParams::default()).unwrap()
            .check_ip("192.168.1.1".parse().unwrap()).is_err());
        assert!(IpPolicy::new(&params(&["10.0.0.1/33"], &[])).is_err());
        assert!(IpPolicy::new(&params(&["no-such-host"], &[])).is_err());
        assert!(IpPolicy::new(&params(&["::1/129"], &[])).is_err());
    }
    #[test]
    fn exclude_overrides_allow_and_no_cross_family_bypass() {
        let p = IpPolicy::new(&params(&["10.20.0.0/24"], &["10.20.0.9"])).unwrap();
        assert!(p.check_ip("10.20.0.5".parse().unwrap()).unwrap());
        assert!(!p.check_ip("10.20.0.9".parse().unwrap()).unwrap());
        assert!(p.check_ip("10.21.0.1".parse().unwrap()).is_err());
        assert!(p.check_ip("::1".parse().unwrap()).is_err());
        let p = IpPolicy::new(&params(&["::1/128"], &[])).unwrap();
        assert!(p.check_ip("::1".parse().unwrap()).unwrap());
        assert!(p.check_ip("127.0.0.1".parse().unwrap()).is_err());
    }
    #[test]
    fn range_requires_full_authorization_with_explicit_exclusions() {
        let p = IpPolicy::new(&params(&["10.20.0.0/24"], &["10.20.0.9"])).unwrap();
        let ips = ["10.20.0.5", "10.20.0.9"].iter().map(|s| s.parse().unwrap()).collect();
        assert_eq!(p.filter_range(ips).unwrap(), vec!["10.20.0.5".parse::<Ipv4Addr>().unwrap()]);
        assert!(p.filter_range(vec!["10.21.0.1".parse().unwrap()]).is_err());
        assert!(p.filter_range(vec!["10.20.0.9".parse().unwrap()]).is_err());
    }
    #[test]
    fn rejects_hostnames_before_network_io() {
        let p = IpPolicy::new(&params(&["127.0.0.1/32"], &[])).unwrap();
        let target = DeviceTarget {
            kind: TransportKind::Tcp, endpoint: "localhost".into(),
            port: Some(502), unit_id: 1, serial: None, serial_mode: None,
        };
        assert!(p.check_device(&target).is_err());
        assert!(p.check_device(&DeviceTarget { endpoint: "127.0.0.1".into(), ..target }).is_ok());
    }
}

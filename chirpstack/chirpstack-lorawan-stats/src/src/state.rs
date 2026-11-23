//! Per Concentratord instance state, shared between the daemon (which writes
//! it) and the collectd exec plugin (which reads it). The state is stored as
//! a simple key=value file, one file per instance.
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::proto::GatewayStats;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct State {
    /// Unix timestamp (seconds) of the last received stats event.
    pub updated_at: u64,
    pub gateway_id: String,

    /// Packet counters, accumulated by the daemon (Concentratord resets
    /// these on every stats interval).
    pub rx_packets_total: u64,
    pub tx_packets_total: u64,

    /// Values below are reported by Concentratord in the stats metadata.
    /// These are None if the Concentratord version does not report them.
    pub rx_airtime_ms_total: Option<u64>,
    pub tx_airtime_ms_total: Option<u64>,
    pub rx_airtime_pct: Option<f64>,
    pub tx_airtime_pct: Option<f64>,
    pub active_devices_1h: Option<u64>,
    pub active_devices_24h: Option<u64>,
}

impl State {
    pub fn update(&mut self, stats: &GatewayStats) {
        fn md<T: std::str::FromStr>(stats: &GatewayStats, key: &str) -> Option<T> {
            stats.metadata.get(key).and_then(|v| v.parse().ok())
        }

        self.updated_at = now();
        self.gateway_id.clone_from(&stats.gateway_id);
        self.rx_packets_total += stats.rx_packets_received_ok as u64;
        self.tx_packets_total += stats.tx_packets_emitted as u64;
        self.rx_airtime_ms_total = md(stats, "rx_airtime_ms_total");
        self.tx_airtime_ms_total = md(stats, "tx_airtime_ms_total");
        self.rx_airtime_pct = md(stats, "rx_airtime_pct");
        self.tx_airtime_pct = md(stats, "tx_airtime_pct");
        self.active_devices_1h = md(stats, "active_devices_1h");
        self.active_devices_24h = md(stats, "active_devices_24h");
    }

    pub fn to_kv(&self) -> String {
        let mut m: BTreeMap<&str, String> = BTreeMap::new();
        m.insert("updated_at", self.updated_at.to_string());
        m.insert("gateway_id", self.gateway_id.clone());
        m.insert("rx_packets_total", self.rx_packets_total.to_string());
        m.insert("tx_packets_total", self.tx_packets_total.to_string());

        let mut opt = |k, v: Option<String>| {
            if let Some(v) = v {
                m.insert(k, v);
            }
        };
        opt(
            "rx_airtime_ms_total",
            self.rx_airtime_ms_total.map(|v| v.to_string()),
        );
        opt(
            "tx_airtime_ms_total",
            self.tx_airtime_ms_total.map(|v| v.to_string()),
        );
        opt(
            "rx_airtime_pct",
            self.rx_airtime_pct.map(|v| format!("{:.2}", v)),
        );
        opt(
            "tx_airtime_pct",
            self.tx_airtime_pct.map(|v| format!("{:.2}", v)),
        );
        opt(
            "active_devices_1h",
            self.active_devices_1h.map(|v| v.to_string()),
        );
        opt(
            "active_devices_24h",
            self.active_devices_24h.map(|v| v.to_string()),
        );

        m.iter().map(|(k, v)| format!("{}={}\n", k, v)).collect()
    }

    pub fn from_kv(s: &str) -> Self {
        let m: BTreeMap<&str, &str> = s
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.trim(), v.trim()))
            .collect();

        fn get<T: std::str::FromStr>(m: &BTreeMap<&str, &str>, k: &str) -> Option<T> {
            m.get(k).and_then(|v| v.parse().ok())
        }

        State {
            updated_at: get(&m, "updated_at").unwrap_or_default(),
            gateway_id: m.get("gateway_id").unwrap_or(&"").to_string(),
            rx_packets_total: get(&m, "rx_packets_total").unwrap_or_default(),
            tx_packets_total: get(&m, "tx_packets_total").unwrap_or_default(),
            rx_airtime_ms_total: get(&m, "rx_airtime_ms_total"),
            tx_airtime_ms_total: get(&m, "tx_airtime_ms_total"),
            rx_airtime_pct: get(&m, "rx_airtime_pct"),
            tx_airtime_pct: get(&m, "tx_airtime_pct"),
            active_devices_1h: get(&m, "active_devices_1h"),
            active_devices_24h: get(&m, "active_devices_24h"),
        }
    }

    /// Atomically write the state to the given path.
    pub fn write(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(self.to_kv().as_bytes())?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_update_and_roundtrip() {
        let mut s = State::default();
        let mut stats = GatewayStats {
            gateway_id: "0102030405060708".into(),
            rx_packets_received_ok: 10,
            tx_packets_emitted: 2,
            metadata: HashMap::from([
                ("rx_airtime_ms_total".into(), "1234".into()),
                ("tx_airtime_ms_total".into(), "567".into()),
                ("rx_airtime_pct".into(), "4.11".into()),
                ("tx_airtime_pct".into(), "1.89".into()),
                ("active_devices_1h".into(), "3".into()),
                ("active_devices_24h".into(), "7".into()),
            ]),
            ..Default::default()
        };

        s.update(&stats);
        stats.rx_packets_received_ok = 5;
        s.update(&stats);

        assert_eq!(15, s.rx_packets_total);
        assert_eq!(4, s.tx_packets_total);
        assert_eq!(Some(1234), s.rx_airtime_ms_total);
        assert_eq!(Some(7), s.active_devices_24h);

        assert_eq!(s, State::from_kv(&s.to_kv()));
    }

    #[test]
    fn test_unpatched_concentratord() {
        // Concentratord without the usage metadata: only packet counters.
        let mut s = State::default();
        s.update(&GatewayStats {
            rx_packets_received_ok: 1,
            ..Default::default()
        });

        let kv = s.to_kv();
        assert!(!kv.contains("airtime"));
        assert!(!kv.contains("active_devices"));
        assert_eq!(s, State::from_kv(&kv));
    }
}

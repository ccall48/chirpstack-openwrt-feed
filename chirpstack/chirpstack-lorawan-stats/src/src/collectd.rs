//! Collectd exec plugin: reads the state files written by the daemon and
//! prints them using the collectd plain-text protocol (PUTVAL).
//!
//! The collectd exec plugin refuses to run programs as root, while the
//! Concentratord IPC sockets are only accessible by root. This is why the
//! ZMQ subscription is handled by the daemon and this command only reads the
//! state files.
use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;

use crate::state::{self, State};

const PLUGIN: &str = "lorawan";

// Concentratord publishes stats every 30 seconds. Values that have not been
// updated within this period are considered stale (e.g. Concentratord is
// stopped) and are not reported.
const MIN_STALE_SECS: u64 = 120;

pub fn run(state_dir: &Path) -> Result<()> {
    let host = env::var("COLLECTD_HOSTNAME").unwrap_or_default();
    let interval = env::var("COLLECTD_INTERVAL").unwrap_or_default();

    if host.is_empty() || interval.is_empty() {
        return Ok(());
    }

    let interval: f64 = interval.parse()?;
    let interval_secs = (interval as u64).max(1);
    let stale_secs = (3 * interval_secs).max(MIN_STALE_SECS);

    loop {
        if let Ok(entries) = fs::read_dir(state_dir) {
            let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            entries.sort_by_key(|e| e.file_name());

            for e in entries {
                let Ok(name) = e.file_name().into_string() else {
                    continue;
                };
                if name.ends_with(".tmp") {
                    continue;
                }
                let Ok(content) = fs::read_to_string(e.path()) else {
                    continue;
                };

                let s = State::from_kv(&content);
                if state::now().saturating_sub(s.updated_at) > stale_secs {
                    continue;
                }

                for line in putval(&host, &name, interval_secs, &s) {
                    println!("{}", line);
                }
            }
        }

        std::thread::sleep(Duration::from_secs(interval_secs));
    }
}

fn putval(host: &str, instance: &str, interval: u64, s: &State) -> Vec<String> {
    // Single slot gateways use the plugin without instance.
    let plugin = if instance == "concentratord" {
        PLUGIN.to_string()
    } else {
        format!("{}-{}", PLUGIN, instance)
    };

    let mut values: Vec<(&str, String)> = vec![
        ("packets-rx", s.rx_packets_total.to_string()),
        ("packets-tx", s.tx_packets_total.to_string()),
    ];

    if let Some(v) = s.active_devices_1h {
        values.push(("count-active_devices_1h", v.to_string()));
    }
    if let Some(v) = s.active_devices_24h {
        values.push(("count-active_devices_24h", v.to_string()));
    }
    // Airtime is reported as a counter (ms); RRD stores the rate (ms/s).
    if let Some(v) = s.rx_airtime_ms_total {
        values.push(("derive-rx_airtime", v.to_string()));
    }
    if let Some(v) = s.tx_airtime_ms_total {
        values.push(("derive-tx_airtime", v.to_string()));
    }

    values
        .into_iter()
        .map(|(t, v)| {
            format!(
                "PUTVAL \"{}/{}/{}\" interval={} N:{}",
                host, plugin, t, interval, v
            )
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_putval() {
        let s = State {
            rx_packets_total: 10,
            tx_packets_total: 2,
            rx_airtime_ms_total: Some(1234),
            tx_airtime_ms_total: Some(567),
            active_devices_1h: Some(3),
            active_devices_24h: Some(7),
            ..Default::default()
        };

        assert_eq!(
            vec![
                "PUTVAL \"gw/lorawan/packets-rx\" interval=30 N:10",
                "PUTVAL \"gw/lorawan/packets-tx\" interval=30 N:2",
                "PUTVAL \"gw/lorawan/count-active_devices_1h\" interval=30 N:3",
                "PUTVAL \"gw/lorawan/count-active_devices_24h\" interval=30 N:7",
                "PUTVAL \"gw/lorawan/derive-rx_airtime\" interval=30 N:1234",
                "PUTVAL \"gw/lorawan/derive-tx_airtime\" interval=30 N:567",
            ],
            putval("gw", "concentratord", 30, &s)
        );

        let s = State {
            rx_packets_total: 1,
            ..Default::default()
        };
        assert_eq!(
            vec![
                "PUTVAL \"gw/lorawan-slot1/packets-rx\" interval=30 N:1",
                "PUTVAL \"gw/lorawan-slot1/packets-tx\" interval=30 N:0",
            ],
            putval("gw", "slot1", 30, &s)
        );
    }
}

//! Subscribes to the event socket of every Concentratord instance and keeps
//! a state file per instance up-to-date.
//!
//! Concentratord binds its event socket as an IPC socket in the socket
//! directory, e.g.:
//!
//! * /tmp/concentratord_event (single slot gateways)
//! * /tmp/concentratord_slot1_event, /tmp/concentratord_slot2_event
//!
//! New sockets are discovered periodically, so it does not matter whether
//! this daemon or Concentratord is started first.
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use prost::Message;

use crate::proto;
use crate::state::State;

const SOCKET_PREFIX: &str = "concentratord";
const SOCKET_SUFFIX: &str = "_event";
const RESCAN_INTERVAL: Duration = Duration::from_secs(30);
const POLL_TIMEOUT_MS: i64 = 1000;

struct Instance {
    name: String,
    socket: zmq::Socket,
    state: State,
}

pub fn run(socket_dir: &Path, state_dir: &Path) -> Result<()> {
    fs::create_dir_all(state_dir)
        .with_context(|| format!("Create state directory: {}", state_dir.display()))?;

    let ctx = zmq::Context::new();
    let mut instances: Vec<Instance> = Vec::new();
    let mut last_scan: Option<Instant> = None;

    loop {
        if last_scan.is_none_or(|v| v.elapsed() >= RESCAN_INTERVAL) {
            last_scan = Some(Instant::now());

            for (name, path) in discover(socket_dir)? {
                if instances.iter().any(|v| v.name == name) {
                    continue;
                }

                let url = format!("ipc://{}", path.display());
                eprintln!(
                    "Subscribing to Concentratord events, instance: {}, url: {}",
                    name, url
                );

                let socket = ctx.socket(zmq::SUB)?;
                socket.connect(&url)?;
                socket.set_subscribe(b"")?;

                instances.push(Instance {
                    name,
                    socket,
                    state: State::default(),
                });
            }
        }

        if instances.is_empty() {
            std::thread::sleep(Duration::from_millis(POLL_TIMEOUT_MS as u64));
            continue;
        }

        let readable: Vec<bool> = {
            let mut items: Vec<zmq::PollItem> = instances
                .iter()
                .map(|v| v.socket.as_poll_item(zmq::POLLIN))
                .collect();
            zmq::poll(&mut items, POLL_TIMEOUT_MS)?;
            items.iter().map(|v| v.is_readable()).collect()
        };

        for (inst, readable) in instances.iter_mut().zip(readable) {
            if !readable {
                continue;
            }

            while let Ok(b) = inst.socket.recv_bytes(zmq::DONTWAIT) {
                if let Err(e) = handle_event(inst, &b, state_dir) {
                    eprintln!(
                        "Handle event error, instance: {}, error: {:#}",
                        inst.name, e
                    );
                }
            }
        }
    }
}

fn handle_event(inst: &mut Instance, b: &[u8], state_dir: &Path) -> Result<()> {
    let event = proto::Event::decode(b).context("Decode event")?;

    if let Some(proto::event::Event::GatewayStats(stats)) = event.event {
        inst.state.update(&stats);
        inst.state
            .write(&state_dir.join(&inst.name))
            .context("Write state")?;
    }

    Ok(())
}

/// Returns the (instance name, socket path) of all Concentratord event
/// sockets in the given directory.
fn discover(socket_dir: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut out: Vec<(String, PathBuf)> = fs::read_dir(socket_dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let file_name = e.file_name().into_string().ok()?;
            let name = instance_name(&file_name)?;
            Some((name, e.path()))
        })
        .collect();
    out.sort();
    Ok(out)
}

/// concentratord_event -> concentratord
/// concentratord_slot1_event -> slot1
pub fn instance_name(file_name: &str) -> Option<String> {
    let middle = file_name
        .strip_prefix(SOCKET_PREFIX)?
        .strip_suffix(SOCKET_SUFFIX)?;

    if middle.is_empty() {
        return Some(SOCKET_PREFIX.to_string());
    }

    let name = middle.strip_prefix('_')?;
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }

    Some(name.to_string())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_instance_name() {
        assert_eq!(
            Some("concentratord".into()),
            instance_name("concentratord_event")
        );
        assert_eq!(
            Some("slot1".into()),
            instance_name("concentratord_slot1_event")
        );
        assert_eq!(
            Some("slot2".into()),
            instance_name("concentratord_slot2_event")
        );
        assert_eq!(None, instance_name("concentratord_command"));
        assert_eq!(None, instance_name("concentratord_slot1_command"));
        assert_eq!(None, instance_name("gateway_relay_event"));
        assert_eq!(None, instance_name("concentratord__event"));
        assert_eq!(None, instance_name("concentratordx_event"));
    }

    #[test]
    fn test_daemon_end_to_end() {
        let base = std::env::temp_dir().join(format!("lorawan-stats-test-{}", std::process::id()));
        let socket_dir = base.join("sockets");
        let state_dir = base.join("state");
        fs::create_dir_all(&socket_dir).unwrap();

        // Fake Concentratord publisher.
        let ctx = zmq::Context::new();
        let publisher = ctx.socket(zmq::PUB).unwrap();
        publisher
            .bind(&format!(
                "ipc://{}",
                socket_dir.join("concentratord_slot1_event").display()
            ))
            .unwrap();

        {
            let (socket_dir, state_dir) = (socket_dir.clone(), state_dir.clone());
            std::thread::spawn(move || run(&socket_dir, &state_dir).unwrap());
        }

        let event = proto::Event {
            event: Some(proto::event::Event::GatewayStats(proto::GatewayStats {
                gateway_id: "0102030405060708".into(),
                rx_packets_received_ok: 4,
                tx_packets_emitted: 1,
                metadata: [
                    ("active_devices_1h", "2"),
                    ("active_devices_24h", "5"),
                    ("rx_airtime_ms_total", "185"),
                    ("tx_airtime_ms_total", "62"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
                ..Default::default()
            })),
        };

        // PUB/SUB drops messages until the subscriber is connected, so keep
        // publishing until the state file appears.
        let state_file = state_dir.join("slot1");
        let start = Instant::now();
        while !state_file.exists() {
            assert!(start.elapsed() < Duration::from_secs(10), "no state file");
            publisher.send(event.encode_to_vec(), 0).unwrap();
            std::thread::sleep(Duration::from_millis(100));
        }

        let s = State::from_kv(&fs::read_to_string(&state_file).unwrap());
        assert_eq!("0102030405060708", s.gateway_id);
        assert_eq!(Some(2), s.active_devices_1h);
        assert_eq!(Some(5), s.active_devices_24h);
        assert_eq!(Some(185), s.rx_airtime_ms_total);
        assert_eq!(Some(62), s.tx_airtime_ms_total);
        assert!(s.rx_packets_total >= 4);

        let _ = fs::remove_dir_all(&base);
    }
}

# chirpstack-lorawan-stats

LoRaWAN usage statistics for ChirpStack Gateway OS:

* Active devices (unique DevAddrs seen in the last hour and in the last 24 hours)
* Uplink and downlink airtime (time-on-air in ms, and % of wall-clock time)
* Uplink (CRC OK) and downlink (emitted) packets

The statistics are shown in LuCI under **Status → Statistics → Graphs →
LoRaWAN**. They are also sent to ChirpStack as gateway stats metadata, which
appears in the gateway properties.

## How it works

```
 chirpstack-concentratord (patched)
   │  GatewayStats event + usage metadata (every stats interval, default 30 s)
   │  ipc:///tmp/concentratord[_slotN]_event
   ├──────────────────────────────► chirpstack-mqtt-forwarder ──► ChirpStack
   ▼
 chirpstack-lorawan-stats daemon      (procd service, root)
   │  writes /var/run/chirpstack-lorawan-stats/<instance>
   ▼
 chirpstack-lorawan-stats collectd    (collectd exec plugin, nobody:nogroup)
   │  PUTVAL ...
   ▼
 collectd RRD ──► luci-app-statistics (lorawan.js graph definition)
```

### Concentratord patch

The metrics are computed in Concentratord, because it sees every uplink and
downlink. The patch is
`chirpstack-concentratord/patches/100-lorawan-usage-stats.patch`, and it:

* adds `libconcentratord/src/airtime.rs`. This calculates uplink time-on-air
  from the received modulation parameters and payload size: LoRa (sub-GHz,
  including low data-rate optimization), LoRa 2.4 GHz, and FSK. LR-FHSS
  uplinks are not counted. It also extracts the DevAddr from unconfirmed and
  confirmed data uplinks (LoRaWAN 1.x). Join requests carry no DevAddr and are
  not counted as active devices.
* records the time-on-air of every downlink emitted by the JIT queue
  (sx1301, sx1302 and 2g4), using the HAL's own time-on-air function.
* adds the following keys to the `GatewayStats.metadata` map:

| Key                    | Description                                                      |
| ---------------------- | ---------------------------------------------------------------- |
| `active_devices_1h`    | Unique DevAddrs seen in the last hour                            |
| `active_devices_24h`   | Unique DevAddrs seen in the last 24 hours (capped at 100000)     |
| `rx_airtime_ms_total`  | Total uplink time-on-air since Concentratord start (ms)          |
| `tx_airtime_ms_total`  | Total downlink time-on-air since Concentratord start (ms)        |
| `rx_airtime_pct`       | Uplink time-on-air during the last stats interval, % of interval |
| `tx_airtime_pct`       | Downlink time-on-air during the last stats interval, % of interval |

Uplink airtime is summed over all channels. On an 8-channel gateway it can
therefore exceed 100%.

All values are held in memory and reset when Concentratord restarts.

### Collector

`chirpstack-lorawan-stats` is a small Rust binary (`src/`) with three
subcommands:

* `daemon`: run by the init script. It subscribes to every
  `/tmp/concentratord*_event` socket and rescans for new sockets every 30 s.
  On each `GatewayStats` event it writes a `key=value` state file per
  instance to `/var/run/chirpstack-lorawan-stats/`. The instance name is
  `concentratord`, or `slot1` / `slot2` on multi-slot gateways.
* `collectd`: run by the collectd exec plugin. It reads the state files once
  per `COLLECTD_INTERVAL` and prints collectd `PUTVAL` lines. Data older than
  3× the interval, with a minimum of 120 s, is skipped, so the graphs show a
  gap when Concentratord is not running.
* `print`: prints the current state files, for debugging.

The work is split into two processes because the collectd exec plugin refuses
to run commands as root, and the Concentratord IPC sockets are owned by root.

collectd values, under plugin `lorawan` (`lorawan-<instance>` for slots):

| Type / instance            | Source                                       |
| -------------------------- | -------------------------------------------- |
| `packets-rx`, `packets-tx` | `rx_packets_received_ok`, `tx_packets_emitted` (accumulated) |
| `count-active_devices_1h`  | `active_devices_1h`                          |
| `count-active_devices_24h` | `active_devices_24h`                         |
| `derive-rx_airtime`        | `rx_airtime_ms_total`                        |
| `derive-tx_airtime`        | `tx_airtime_ms_total`                        |

The airtime graph shows the derive rate (ms/s) divided by 10, which gives a
percentage.

## Installed files

| Path | Purpose |
| ---- | ------- |
| `/usr/bin/chirpstack-lorawan-stats` | Collector binary |
| `/etc/init.d/chirpstack-lorawan-stats` | procd service (`daemon`) |
| `/etc/uci-defaults/90-configure-chirpstack-lorawan-stats` | Enables `luci_statistics.collectd_exec` and adds the exec input (idempotent) |
| `/www/luci-static/resources/statistics/rrdtool/definitions/lorawan.js` | LuCI Statistics graph definition |

## Requirements and limitations

* ChirpStack gateway properties only work with the **MQTT forwarder**. The UDP
  forwarder (Semtech UDP protocol) does not carry the stats metadata. The LuCI
  graphs work with either forwarder.
* The active device count counts DevAddrs, not DevEUIs. It includes every
  device the gateway hears, including devices on other networks.

## Verifying on a gateway

```sh
/etc/init.d/chirpstack-lorawan-stats status
chirpstack-lorawan-stats print
uci show luci_statistics | grep -A3 collectd_exec_input
ls /tmp/rrd/*/lorawan*/
```

## Development

```sh
cd src
cargo test
cargo clippy
```

The patch is generated with `git diff` against the Concentratord tag that
`chirpstack-concentratord/Makefile` builds (currently `v4.7.1`). Regenerate it
when the version is bumped.

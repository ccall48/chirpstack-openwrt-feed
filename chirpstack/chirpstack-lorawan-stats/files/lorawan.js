"use strict";
"require baseclass";

return baseclass.extend({
  title: _("LoRaWAN"),

  rrdargs(graph, host, plugin, plugin_instance, dtype) {
    return [
      {
        title: "%H: Active devices",
        vlabel: "Devices",
        y_min: "0",
        number_format: "%5.0lf",
        data: {
          instances: {
            count: ["active_devices_1h", "active_devices_24h"],
          },
          options: {
            count_active_devices_1h: {
              title: "Last hour",
              overlay: true,
              noarea: true,
              color: "00a000",
            },
            count_active_devices_24h: {
              title: "Last 24 hours",
              overlay: true,
              noarea: true,
              color: "0000ff",
            },
          },
        },
      },
      {
        title: "%H: Airtime",
        vlabel: "% of time",
        y_min: "0",
        number_format: "%5.2lf%%",
        data: {
          instances: {
            derive: ["rx_airtime", "tx_airtime"],
          },
          options: {
            // Stored as ms airtime / s, divide by 10 for percent.
            derive_rx_airtime: {
              title: "Uplink (sum over all channels)",
              transform_rpn: "10,/",
              overlay: true,
              color: "0000ff",
            },
            derive_tx_airtime: {
              title: "Downlink",
              transform_rpn: "10,/",
              overlay: true,
              color: "ff0000",
            },
          },
        },
      },
      {
        title: "%H: Packets",
        vlabel: "Packets / min",
        y_min: "0",
        number_format: "%5.1lf",
        data: {
          instances: {
            packets: ["rx", "tx"],
          },
          options: {
            packets_rx: {
              title: "Uplink (CRC OK)",
              transform_rpn: "60,*",
              overlay: true,
              color: "0000ff",
            },
            packets_tx: {
              title: "Downlink (emitted)",
              transform_rpn: "60,*",
              overlay: true,
              color: "ff0000",
            },
          },
        },
      },
    ];
  },
});

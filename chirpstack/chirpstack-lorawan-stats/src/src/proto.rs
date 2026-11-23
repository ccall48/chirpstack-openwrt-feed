//! Minimal subset of the ChirpStack gw.proto messages, needed to decode the
//! Concentratord stats events. Using hand-written prost structs (instead of
//! the full chirpstack_api crate) keeps the binary small. Unknown fields are
//! ignored by prost.
use std::collections::HashMap;

#[derive(Clone, PartialEq, prost::Message)]
pub struct Event {
    #[prost(oneof = "event::Event", tags = "1, 2")]
    pub event: Option<event::Event>,
}

pub mod event {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Event {
        /// Uplink frame (not decoded).
        #[prost(bytes = "vec", tag = "1")]
        UplinkFrame(Vec<u8>),

        #[prost(message, tag = "2")]
        GatewayStats(super::GatewayStats),
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct GatewayStats {
    #[prost(uint32, tag = "5")]
    pub rx_packets_received: u32,

    #[prost(uint32, tag = "6")]
    pub rx_packets_received_ok: u32,

    #[prost(uint32, tag = "7")]
    pub tx_packets_received: u32,

    #[prost(uint32, tag = "8")]
    pub tx_packets_emitted: u32,

    #[prost(map = "string, string", tag = "10")]
    pub metadata: HashMap<String, String>,

    #[prost(string, tag = "17")]
    pub gateway_id: String,
}

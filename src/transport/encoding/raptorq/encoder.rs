// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::transport::Encoder;
use std::collections::HashMap;

use crate::encoding::{message::Message, payload::BroadcastPayload};

use super::super::Configurable;

const DEFAULT_MIN_REPAIR_PACKETS_PER_BLOCK: u32 = 5;
const DEFAULT_MTU: u16 = 1400;
const DEFAULT_FEQ_REDUNDANCY: f32 = 0.15;

use raptorq::Encoder as ExtEncoder;

pub(crate) struct RaptorQEncoder {
    min_repair_packets_per_block: u32,
    mtu: u16,
    fec_redundancy: f32,
}

impl Configurable for RaptorQEncoder {
    fn configure(conf: &HashMap<String, String>) -> Self {
        let min_repair_packets_per_block = conf
            .get("min_repair_packets_per_block")
            .map(|s| s.parse().ok())
            .flatten()
            .unwrap_or(DEFAULT_MIN_REPAIR_PACKETS_PER_BLOCK);

        let mtu = conf
            .get("mtu")
            .map(|s| s.parse().ok())
            .flatten()
            .unwrap_or(DEFAULT_MTU);

        let fec_redundancy = conf
            .get("fec_redundancy")
            .map(|s| s.parse().ok())
            .flatten()
            .unwrap_or(DEFAULT_FEQ_REDUNDANCY);

        Self {
            min_repair_packets_per_block,
            mtu,
            fec_redundancy,
        }
    }
    fn default_configuration() -> HashMap<String, String> {
        let mut conf = HashMap::new();
        conf.insert(
            "min_repair_packets_per_block".to_string(),
            DEFAULT_MIN_REPAIR_PACKETS_PER_BLOCK.to_string(),
        );
        conf.insert("mtu".to_string(), DEFAULT_MTU.to_string());
        conf
    }
}

impl Encoder for RaptorQEncoder {
    fn encode<'msg>(&self, msg: Message) -> Vec<Message> {
        if let Message::Broadcast(header, payload) = msg {
            let encoder =
                ExtEncoder::with_defaults(&payload.gossip_frame, self.mtu);
            let mut transmission_info =
                encoder.get_config().serialize().to_vec();

            let mut base_packet = payload.generate_uid().to_vec();
            base_packet.append(&mut transmission_info);

            let mut repair_packets = (payload.gossip_frame.len() as f32
                * self.fec_redundancy
                / self.mtu as f32) as u32;
            if repair_packets < self.min_repair_packets_per_block {
                repair_packets = self.min_repair_packets_per_block
            }

            encoder
                .get_encoded_packets(repair_packets)
                .iter()
                .map(|encoded_packet| {
                    let mut packet_with_uid = base_packet.clone();
                    packet_with_uid.append(&mut encoded_packet.serialize());
                    Message::Broadcast(
                        header,
                        BroadcastPayload {
                            height: payload.height,
                            gossip_frame: packet_with_uid,
                        },
                    )
                })
                .collect()
        } else {
            vec![msg]
        }
    }
}

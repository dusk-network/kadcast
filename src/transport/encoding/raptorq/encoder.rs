// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::transport::Encoder;
use std::collections::HashMap;

use crate::encoding::{message::Message, payload::BroadcastPayload};

use super::super::Configurable;

const DEFAULT_REPAIR_PACKETS_PER_BLOCK: u32 = 15;
const DEFAULT_MAX_CHUNK_SIZE: u16 = 1024;

use raptorq::Encoder as ExtEncoder;

pub(crate) struct RaptorQEncoder {
    repair_packets_per_block: u32,
    max_chunk_size: u16,
}

impl Configurable for RaptorQEncoder {
    fn configure(conf: &HashMap<String, String>) -> Self {
        let repair_packets_per_block = conf
            .get("repair_packets_per_block")
            .map(|s| s.parse().ok())
            .flatten()
            .unwrap_or(DEFAULT_REPAIR_PACKETS_PER_BLOCK);

        let max_chunk_size = conf
            .get("max_chunk_size")
            .map(|s| s.parse().ok())
            .flatten()
            .unwrap_or(DEFAULT_MAX_CHUNK_SIZE);
        Self {
            repair_packets_per_block,
            max_chunk_size,
        }
    }
    fn default_configuration() -> HashMap<String, String> {
        let mut conf = HashMap::new();
        conf.insert(
            "repair_packets_per_block".to_string(),
            DEFAULT_REPAIR_PACKETS_PER_BLOCK.to_string(),
        );
        conf.insert(
            "max_chunk_size".to_string(),
            DEFAULT_MAX_CHUNK_SIZE.to_string(),
        );
        conf
    }
}

impl Encoder for RaptorQEncoder {
    fn encode<'msg>(&self, msg: Message) -> Vec<Message> {
        if let Message::Broadcast(header, payload) = msg {
            let uid = payload.generate_uid();
            let encoder = ExtEncoder::with_defaults(
                &payload.gossip_frame,
                self.max_chunk_size,
            );
            encoder
                .get_encoded_packets(self.repair_packets_per_block)
                .iter()
                .map(|encoded_packet| {
                    let mut packet_with_uid = uid.to_vec();
                    let mut transmission_info =
                        encoder.get_config().serialize().to_vec();
                    packet_with_uid.append(&mut transmission_info);
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

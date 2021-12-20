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
const MAX_CHUNK_SIZE: u16 = 1024;

use raptorq::Encoder as ExtEncoder;

pub(crate) struct RaptorQEncoder {}

impl Configurable for RaptorQEncoder {
    fn configure(_: HashMap<String, String>) -> Self {
        Self {}
    }
}

impl Encoder for RaptorQEncoder {
    fn encode<'msg>(&self, msg: Message) -> Vec<Message> {
        if let Message::Broadcast(header, payload) = msg {
            let uid = payload.generate_uid();
            let encoder = ExtEncoder::with_defaults(
                &payload.gossip_frame,
                MAX_CHUNK_SIZE,
            );
            encoder
                .get_encoded_packets(DEFAULT_REPAIR_PACKETS_PER_BLOCK)
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

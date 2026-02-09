// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::io;

use crate::encoding::message::Message;
use crate::encoding::payload::BroadcastPayload;
use crate::transport::Encoder;
use crate::transport::encoding::Configurable;

const DEFAULT_MIN_REPAIR_PACKETS_PER_BLOCK: u32 = 5;
const DEFAULT_MTU: u16 = 1300;

pub(crate) const MAX_MTU: u16 = 8192;
pub(crate) const MIN_MTU: u16 = 1296;

const DEFAULT_FEQ_REDUNDANCY: f32 = 0.15;

use raptorq::Encoder as ExtEncoder;
use serde_derive::{Deserialize, Serialize};
use tracing::debug;

pub struct RaptorQEncoder {
    conf: RaptorQEncoderConf,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct RaptorQEncoderConf {
    min_repair_packets_per_block: u32,
    mtu: u16,
    fec_redundancy: f32,
}

impl Default for RaptorQEncoderConf {
    fn default() -> Self {
        RaptorQEncoderConf {
            fec_redundancy: DEFAULT_FEQ_REDUNDANCY,
            min_repair_packets_per_block: DEFAULT_MIN_REPAIR_PACKETS_PER_BLOCK,
            mtu: DEFAULT_MTU,
        }
    }
}

impl Configurable for RaptorQEncoder {
    type TConf = RaptorQEncoderConf;

    fn default_configuration() -> Self::TConf {
        RaptorQEncoderConf::default()
    }
    fn configure(conf: &Self::TConf) -> Self {
        let mut conf = *conf;
        let mtu = conf.mtu;
        if mtu > MAX_MTU {
            tracing::warn!("MTU={mtu} too high, changing to {DEFAULT_MTU}");
            conf.mtu = DEFAULT_MTU;
        }
        if mtu < MIN_MTU {
            tracing::warn!("MTU={mtu} too low, changing to {DEFAULT_MTU}");
            conf.mtu = DEFAULT_MTU;
        }
        Self { conf }
    }
}

impl Encoder for RaptorQEncoder {
    fn encode<'msg>(&self, msg: Message) -> io::Result<Vec<Message>> {
        if let Message::Broadcast(header, payload, ..) = msg {
            let encoder =
                ExtEncoder::with_defaults(&payload.gossip_frame, self.conf.mtu);
            let transmission_info = encoder.get_config().serialize();

            let ray_id = payload.generate_ray_id()?;
            let raptorq_header = [&ray_id[..], &transmission_info].concat();

            debug!(
                event = "Start encoding payload",
                ray = hex::encode(ray_id),
                encode_info = hex::encode(transmission_info)
            );

            let mut repair_packets =
                (payload.gossip_frame.len() as f32 * self.conf.fec_redundancy
                    / self.conf.mtu as f32) as u32;
            if repair_packets < self.conf.min_repair_packets_per_block {
                repair_packets = self.conf.min_repair_packets_per_block
            }

            let messages = encoder
                .get_encoded_packets(repair_packets)
                .iter()
                .map(|encoded_packet| {
                    let mut gossip_frame = raptorq_header.clone();
                    gossip_frame.append(&mut encoded_packet.serialize());
                    Message::Broadcast(
                        header,
                        BroadcastPayload {
                            height: payload.height,
                            gossip_frame,
                        },
                        ray_id,
                    )
                })
                .collect();
            Ok(messages)
        } else {
            Ok(vec![msg])
        }
    }
}

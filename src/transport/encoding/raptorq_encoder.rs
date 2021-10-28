// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::{collections::HashMap, convert::TryInto};

use blake2::{Blake2s, Digest};
use raptorq::{Decoder, Encoder, EncodingPacket};

use crate::{
    encoding::{message::Message, payload::BroadcastPayload, Marshallable},
    K_CHUNK_SIZE,
};

pub(crate) struct RaptorQEncoder {
    cache: HashMap<[u8; 32], CacheStatus>,
}

impl RaptorQEncoder {
    pub(crate) fn new() -> Self {
        RaptorQEncoder {
            cache: HashMap::new(),
        }
    }
}
enum CacheStatus {
    Pending(Decoder),
    Completed,
}

impl BroadcastPayload {
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];
        self.marshal_binary(&mut bytes).unwrap();
        bytes
    }

    fn uid(&self) -> [u8; 32] {
        let mut hasher = Blake2s::new();
        hasher.update(self.bytes());
        hasher
            .finalize()
            .as_slice()
            .try_into()
            .expect("Wrong length")
    }
}

impl super::Encoder for RaptorQEncoder {
    fn encode<'msg>(&self, msg: Message) -> Vec<Message> {
        if let Message::Broadcast(header, payload) = msg {
            let uid = payload.uid();
            let encoder =
                Encoder::with_defaults(&payload.gossip_frame, K_CHUNK_SIZE);
            encoder
                .get_encoded_packets(15)
                .iter()
                .map(|packet| {
                    let mut packet_with_uid = uid.to_vec();
                    packet_with_uid.append(&mut packet.serialize());
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

    fn decode(&mut self, message: Message) -> Option<Message> {
        if let Message::Broadcast(header, payload) = message {
            let uid = payload.uid();
            let status = match self.cache.entry(uid) {
                std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(CacheStatus::Pending(Decoder::new(
                        Encoder::with_defaults(&[], K_CHUNK_SIZE).get_config(),
                    )))
                }
            };
            let decoded = match status {
                CacheStatus::Completed => None,
                CacheStatus::Pending(decoder) => decoder
                    .decode(EncodingPacket::deserialize(&payload.bytes()[32..]))
                    .map(|decoded| {
                        Message::Broadcast(
                            header,
                            BroadcastPayload {
                                height: payload.height,
                                gossip_frame: decoded,
                            },
                        )
                    }),
            };
            if let Some(decoded) = decoded {
                self.cache.insert(uid, CacheStatus::Completed);
                Some(decoded)
            } else {
                None
            }
        } else {
            Some(message)
        }
    }
}

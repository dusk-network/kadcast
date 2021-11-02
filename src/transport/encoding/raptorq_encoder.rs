// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::{collections::HashMap, convert::TryInto};

use blake2::{Blake2s, Digest};
use raptorq::{
    Decoder, Encoder, EncodingPacket, ObjectTransmissionInformation,
};
use tracing::warn;

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
    Receiving(Decoder),
    Processed,
}

struct ChunkedPayload<'a>(&'a BroadcastPayload);

impl BroadcastPayload {
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];
        self.marshal_binary(&mut bytes).unwrap();
        bytes
    }
    fn generate_uid(&self) -> [u8; 32] {
        let mut hasher = Blake2s::new();
        hasher.update(&self.bytes()[1..]);
        hasher
            .finalize()
            .as_slice()
            .try_into()
            .expect("Wrong length")
    }
}
impl<'a> ChunkedPayload<'a> {
    fn uid(&self) -> [u8; 32] {
        let mut uid: [u8; 32] = Default::default();
        uid.copy_from_slice(&self.0.gossip_frame[0..32]);
        uid
    }

    fn transmission_info(&self) -> ObjectTransmissionInformation {
        let mut transmission_info: [u8; 12] = Default::default();
        transmission_info.copy_from_slice(&self.0.gossip_frame[32..44]);
        ObjectTransmissionInformation::deserialize(&transmission_info)
    }

    fn encoded_chunk(&self) -> &[u8] {
        &self.0.gossip_frame[44..]
    }

    fn safe_uid(&self) -> [u8; 32] {
        let mut hasher = Blake2s::new();
        let uid = &self.0.gossip_frame[0..32];
        let transmission_info = &self.0.gossip_frame[32..44];
        hasher.update(uid);

        // Why do we need transmission info?
        //
        // Transmission info should be sent over a reliable channel, because
        // it is critical to decode packets.
        // Since it is sent over UDP alongside the encoded chunked bytes,
        // corrupted transmission info can be received.
        // If the corrupted info is part of the first received chunk, no message
        // can ever be decoded.
        hasher.update(transmission_info);
        hasher
            .finalize()
            .as_slice()
            .try_into()
            .expect("Wrong length")
    }
}

impl super::Encoder for RaptorQEncoder {
    fn encode<'msg>(msg: Message) -> Vec<Message> {
        if let Message::Broadcast(header, payload) = msg {
            let uid = payload.generate_uid();
            let encoder =
                Encoder::with_defaults(&payload.gossip_frame, K_CHUNK_SIZE);
            encoder
                .get_encoded_packets(15)
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

    fn decode(&mut self, message: Message) -> Option<Message> {
        if let Message::Broadcast(header, payload) = message {
            let chunked = ChunkedPayload(&payload);
            let uid = chunked.safe_uid();

            // Perform a `match` on the cache entry against the uid.
            let status = match self.cache.entry(uid) {
                // Cache status exists: return it
                std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),

                // Cache status not found: creates a new entry with
                // CacheStatus::Receiving status and binds a new Decoder with
                // the received transmission information
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(CacheStatus::Receiving(Decoder::new(
                        chunked.transmission_info(),
                    )))
                }
            };

            match status {
                // Avoid to repropagate already processed messages
                CacheStatus::Processed => None,
                CacheStatus::Receiving(decoder) => decoder
                    .decode(EncodingPacket::deserialize(
                        chunked.encoded_chunk(),
                    ))
                    // If decoded successfully, create the new BroadcastMessage
                    .and_then(|decoded| {
                        let payload = BroadcastPayload {
                            height: payload.height,
                            gossip_frame: decoded,
                        };
                        // Perform sanity check
                        match chunked.uid() == payload.generate_uid() {
                            true => Some(Message::Broadcast(header, payload)),
                            _ => {
                                warn!("Invalid message decoded");
                                None
                            }
                        }
                    })
                    // If the message is succesfully decoded, update the cache
                    // with new status. This will drop useless Decoder and avoid
                    // to propagate already processed messages
                    .map(|a| {
                        self.cache.insert(uid, CacheStatus::Processed);
                        a
                    }),
            }
        } else {
            Some(message)
        }
    }
}
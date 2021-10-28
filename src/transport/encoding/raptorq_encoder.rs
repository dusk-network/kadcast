// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::convert::TryInto;

use blake2::{Blake2s, Digest};
use raptorq::Encoder;

use crate::{K_CHUNK_SIZE, encoding::{
    message::Message, payload::BroadcastPayload, Marshallable,
}};

pub(crate) struct RaptorQEncoder {}

impl BroadcastPayload {
    fn uid(&self) -> [u8; 32] {
        let mut bytes = vec![];
        self.marshal_binary(&mut bytes).unwrap();
        let mut hasher = Blake2s::new();
        hasher.update(bytes);
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
            let encoder = Encoder::with_defaults(&payload.gossip_frame, K_CHUNK_SIZE);
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

    fn decode(&self, chunk: Message) -> Option<Message> {
        if let Message::Broadcast(header, payload) = chunk {
            Some(Message::Broadcast(header, payload))
        } else {
            Some(chunk)
        }
    }
}

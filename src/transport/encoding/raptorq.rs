// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::convert::TryInto;

use blake2::{Blake2s, Digest};
use raptorq::ObjectTransmissionInformation;

use crate::encoding::{payload::BroadcastPayload, Marshallable};

mod decoder;
mod encoder;

pub(crate) use decoder::RaptorQDecoder;
pub(crate) use encoder::RaptorQEncoder;

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

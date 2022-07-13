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
    fn uid(&self) -> &[u8] {
        &self.0.gossip_frame[0..32]
    }

    fn transmission_info(&self) -> ObjectTransmissionInformation {
        let slice = &self.0.gossip_frame[32..44];
        let transmission_info: &[u8; 12] =
            slice.try_into().expect("slice with incorrect length");
        ObjectTransmissionInformation::deserialize(transmission_info)
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

#[cfg(test)]
mod tests {

    use std::time::Instant;

    use crate::encoding::{message::Message, payload::BroadcastPayload};
    use crate::peer::PeerNode;
    use crate::transport::encoding::{
        Configurable, Decoder, Encoder, TransportDecoder, TransportEncoder,
    };

    #[test]
    fn test_encode() {
        #[cfg(not(debug_assertions))]
        let mut data: Vec<u8> = vec![0; 3_000_000];

        #[cfg(debug_assertions)]
        let mut data: Vec<u8> = vec![0; 100_000];

        for i in 0..data.len() {
            data[i] = rand::Rng::gen(&mut rand::thread_rng());
        }
        let peer = PeerNode::generate("192.168.0.1:666");
        let header = peer.as_header();
        let payload = BroadcastPayload {
            height: 255,
            gossip_frame: data,
        };
        println!("orig payload len {}", payload.bytes().len());
        let message = Message::Broadcast(header, payload);
        let message_bytes = message.bytes();
        println!("orig message len {}", message_bytes.len());
        let start = Instant::now();
        let encoder = TransportEncoder::configure(
            &TransportEncoder::default_configuration(),
        );
        let chunks = encoder.encode(message);
        println!("Encoded in: {:?}", start.elapsed());
        println!("encoded chunks {}", chunks.len());
        let start = Instant::now();
        let mut decoder = TransportDecoder::configure(
            &TransportDecoder::default_configuration(),
        );
        let mut decoded = None;
        let mut i = 0;
        let mut sizetotal = 0;
        for chunk in chunks {
            // println!("chunk {:?}", chunk);
            i = i + 1;
            sizetotal += chunk.bytes().len();
            if let Some(d) = decoder.decode(chunk) {
                decoded = Some(d);
                println!("Decoder after {} messages ", i);
                break;
            }
        }
        println!("Decoded in: {:?}", start.elapsed());
        println!("avg chunks size {}", sizetotal / i);
        assert_eq!(decoded.unwrap().bytes(), message_bytes, "Unable to decode");
    }
}

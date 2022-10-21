// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::convert::{TryFrom, TryInto};
use std::io::{self, ErrorKind};

use blake2::{Blake2s256, Digest};
use raptorq::ObjectTransmissionInformation;

use crate::encoding::{payload::BroadcastPayload, Marshallable};

mod decoder;
mod encoder;

pub(crate) use decoder::RaptorQDecoder;
pub(crate) use encoder::RaptorQEncoder;

struct ChunkedPayload<'a>(&'a BroadcastPayload);

// ObjectTransmissionInformation Size (Raptorq header)
const TRANSMISSION_INFO_SIZE: usize = 12;

// UID Size (Blake2s256)
const UID_SIZE: usize = 32;

// EncodingPacket min size (RaptorQ packet)
const MIN_ENCODING_PACKET_SIZE: usize = 5;

const MIN_CHUNKED_SIZE: usize =
    UID_SIZE + TRANSMISSION_INFO_SIZE + MIN_ENCODING_PACKET_SIZE;

impl<'a> TryFrom<&'a BroadcastPayload> for ChunkedPayload<'a> {
    type Error = io::Error;
    fn try_from(value: &'a BroadcastPayload) -> Result<Self, Self::Error> {
        if value.gossip_frame.len() < MIN_CHUNKED_SIZE {
            Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                "Chunked payload too short",
            ))
        } else {
            Ok(ChunkedPayload(value))
        }
    }
}

impl BroadcastPayload {
    fn bytes(&self) -> io::Result<Vec<u8>> {
        let mut bytes = vec![];
        self.marshal_binary(&mut bytes)?;
        Ok(bytes)
    }
    fn generate_uid(&self) -> io::Result<[u8; UID_SIZE]> {
        let mut hasher = Blake2s256::new();
        // Remove the kadcast `height` field from the hash
        hasher.update(&self.bytes()?[1..]);
        Ok(hasher.finalize().into())
    }
}
impl<'a> ChunkedPayload<'a> {
    fn uid(&self) -> &[u8] {
        &self.0.gossip_frame[0..UID_SIZE]
    }

    fn transmission_info(&self) -> ObjectTransmissionInformation {
        let slice =
            &self.0.gossip_frame[UID_SIZE..(UID_SIZE + TRANSMISSION_INFO_SIZE)];
        let transmission_info: &[u8; TRANSMISSION_INFO_SIZE] =
            slice.try_into().expect("slice to be length 12");
        ObjectTransmissionInformation::deserialize(transmission_info)
    }

    fn encoded_chunk(&self) -> &[u8] {
        &self.0.gossip_frame[(UID_SIZE + TRANSMISSION_INFO_SIZE)..]
    }

    fn safe_uid(&self) -> [u8; UID_SIZE] {
        let mut hasher = Blake2s256::new();

        let uid = &self.0.gossip_frame[0..UID_SIZE];
        let transmission_info =
            &self.0.gossip_frame[UID_SIZE..(UID_SIZE + TRANSMISSION_INFO_SIZE)];
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
        hasher.finalize().into()
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

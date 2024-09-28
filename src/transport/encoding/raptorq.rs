// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::convert::{TryFrom, TryInto};
use std::io::{self, ErrorKind};

use blake2::{Blake2s256, Digest};
use safe::{SafeObjectTransmissionInformation, TransmissionInformationError};

use crate::encoding::{payload::BroadcastPayload, Marshallable};

mod decoder;
mod encoder;
mod safe;

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

    fn transmission_info(
        &self,
        max_udp_len: u64,
    ) -> Result<SafeObjectTransmissionInformation, TransmissionInformationError>
    {
        let slice =
            &self.0.gossip_frame[UID_SIZE..(UID_SIZE + TRANSMISSION_INFO_SIZE)];
        let info = SafeObjectTransmissionInformation::try_from(slice)?;
        match info.inner.transfer_length() < max_udp_len {
            true => Ok(info),
            false => Err(TransmissionInformationError::TransferLengthExceeded),
        }
    }

    fn encoded_chunk(&self) -> &[u8] {
        &self.0.gossip_frame[(UID_SIZE + TRANSMISSION_INFO_SIZE)..]
    }

    fn uid_with_info(&self) -> [u8; UID_SIZE + TRANSMISSION_INFO_SIZE] {
        let uid = &self.0.gossip_frame[0..UID_SIZE + TRANSMISSION_INFO_SIZE];

        // Why do we need transmission info?
        //
        // Transmission info should be sent over a reliable channel, because
        // it is critical to decode packets.
        // Since it is sent over UDP alongside the encoded chunked bytes,
        // corrupted transmission info can be received.
        // If the corrupted info is part of the first received chunk, no
        // message can ever be decoded.
        //
        // ** UPDATE:
        // Since the correctness of an UDP packet is already guaranteed by OS
        // checksum checks, Hashing has been removed in order to increase the
        // decoding performance.
        uid.try_into().expect("slice to be length 44")
    }
}

#[cfg(test)]
mod tests {

    use std::time::Instant;

    use io::{BufWriter, Cursor};

    use super::*;
    use crate::encoding::message::Message;
    use crate::peer::PeerNode;
    use crate::tests::Result;
    use crate::transport::encoding::{
        Configurable, Decoder, Encoder, TransportDecoder, TransportEncoder,
    };
    #[test]
    fn test_encode_raptorq() -> Result<()> {
        #[cfg(not(debug_assertions))]
        let mut data = vec![0; 3_000_000];

        #[cfg(debug_assertions)]
        let mut data = vec![0; 100_000];

        for i in 0..data.len() {
            data[i] = rand::Rng::gen(&mut rand::thread_rng());
        }
        let peer = PeerNode::generate("192.168.0.1:666", 0)?;
        let header = peer.to_header();
        let payload = BroadcastPayload {
            height: 255,
            gossip_frame: data,
            ray: vec![],
        };
        println!("orig payload len {}", payload.bytes()?.len());
        let message = Message::Broadcast(header, payload);
        let message_bytes = message.bytes()?;
        println!("orig message len {}", message_bytes.len());
        let start = Instant::now();
        let encoder = TransportEncoder::configure(
            &TransportEncoder::default_configuration(),
        );
        let chunks = encoder.encode(message)?;
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
            sizetotal += chunk.bytes()?.len();
            if let Some(d) = decoder.decode(chunk).unwrap() {
                decoded = Some(d);
                println!("Decoder after {} messages ", i);
                break;
            }
        }
        println!("Decoded in: {:?}", start.elapsed());
        println!("avg chunks size {}", sizetotal / i);
        assert_eq!(
            decoded.unwrap().bytes()?,
            message_bytes,
            "Unable to decode"
        );
        Ok(())
    }

    #[test]
    fn test_encode_raptorq_junk() -> Result<()> {
        #[cfg(not(debug_assertions))]
        const DATA_LEN: usize = 3_000_000;

        #[cfg(debug_assertions)]
        const DATA_LEN: usize = 100_00;

        let mut data = vec![0; DATA_LEN];

        for i in 0..DATA_LEN {
            data[i] = rand::Rng::gen(&mut rand::thread_rng());
        }
        let peer = PeerNode::generate("192.168.0.1:666", 0)?;
        let header = peer.to_header();
        let payload = BroadcastPayload {
            height: 255,
            gossip_frame: data,
            ray: vec![],
        };
        println!("orig payload len {}", payload.bytes()?.len());
        let message = Message::Broadcast(header, payload);
        let message_bytes = message.bytes()?;
        println!("orig message len {}", message_bytes.len());
        let start = Instant::now();
        let encoder = TransportEncoder::configure(
            &TransportEncoder::default_configuration(),
        );
        let chunks = encoder.encode(message)?;
        println!("Encoded in: {:?}", start.elapsed());
        println!("encoded chunks {}", chunks.len());
        let mut decoder = TransportDecoder::configure(
            &TransportDecoder::default_configuration(),
        );
        let junks_messages = 100;
        println!("start spamming with {junks_messages} junk messages");
        let mut decoded = None;
        for _ in 0..junks_messages {
            let mut gossip_frame = vec![];
            for _ in 0..DATA_LEN {
                gossip_frame.push(rand::Rng::gen(&mut rand::thread_rng()));
            }
            let msg = Message::Broadcast(
                header,
                BroadcastPayload {
                    height: 255,
                    gossip_frame,
                    ray: vec![],
                },
            );
            if let Ok(Some(_)) = decoder.decode(msg) {
                panic!("This should be junk data");
            }
        }
        let mut i = 0;
        let mut sizetotal = 0;
        println!("start decoding (with additional junk messages)");
        let start = Instant::now();
        let mut junk = 0;
        for chunk in chunks {
            i = i + 1;
            sizetotal += chunk.bytes()?.len();
            let cloned_chunk = clone_and_corrupt_msg(&chunk)?;
            for _ in 0..1000 {
                let cloned_chunk = clone_and_corrupt_msg(&cloned_chunk)?;
                if let Ok(Some(_)) = decoder.decode(cloned_chunk) {
                    panic!("This should be junk data");
                }
                junk += 1;
            }
            if let Some(d) = decoder.decode(chunk).unwrap() {
                decoded = Some(d);
                println!("Decoder after {i} messages (and {junk} messages) ");
                break;
            }
        }
        println!("Decoded in: {:?}", start.elapsed());
        println!("avg chunks size {}", sizetotal / i);
        assert_eq!(
            decoded.unwrap().bytes()?,
            message_bytes,
            "Unable to decode"
        );
        Ok(())
    }

    use std::io::BufReader;
    use std::io::Read;
    use std::io::Seek;
    fn clone_and_corrupt_msg(message: &Message) -> Result<Message> {
        let mut c = Cursor::new(Vec::new());
        let mut writer = BufWriter::new(c);
        message.marshal_binary(&mut writer)?;
        c = writer.into_inner()?;
        let mut bytes = vec![];
        c.seek(std::io::SeekFrom::Start(0))?;
        c.read_to_end(&mut bytes)?;
        for i in 44..bytes.len() {
            bytes[i] = rand::Rng::gen(&mut rand::thread_rng());
        }
        let c = Cursor::new(bytes);
        let mut reader = BufReader::new(c);
        let msg = Message::unmarshal_binary(&mut reader)?;
        Ok(msg)
    }
}

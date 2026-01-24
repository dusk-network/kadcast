// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::io;
use std::time::{Duration, Instant};

use raptorq::{Decoder as ExtDecoder, EncodingPacket};
use serde::{Deserialize, Serialize};
use tracing::{debug, trace, warn};

use super::{ChunkedPayload, RAY_ID_SIZE, TRANSMISSION_INFO_SIZE};
use crate::encoding::message::Message;
use crate::encoding::payload::BroadcastPayload;
use crate::transport::encoding::Configurable;
use crate::transport::Decoder;

const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(60);
const DEFAULT_CACHE_PRUNE_EVERY: Duration = Duration::from_secs(30);

const DEFAULT_MAX_UDP_LEN: u64 = 10 * 1_024 * 1_024;

pub struct RaptorQDecoder {
    cache: BTreeMap<[u8; RAY_ID_SIZE], CacheStatus>,
    last_pruned: Instant,
    conf: RaptorQDecoderConf,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct RaptorQDecoderConf {
    #[serde(with = "humantime_serde")]
    pub cache_ttl: Duration,
    #[serde(with = "humantime_serde")]
    pub cache_prune_every: Duration,

    #[serde(default = "default_max_udp_len")]
    pub max_udp_len: u64,
}

const fn default_max_udp_len() -> u64 {
    DEFAULT_MAX_UDP_LEN
}

impl Configurable for RaptorQDecoder {
    type TConf = RaptorQDecoderConf;
    fn default_configuration() -> Self::TConf {
        RaptorQDecoderConf {
            cache_prune_every: DEFAULT_CACHE_PRUNE_EVERY,
            cache_ttl: DEFAULT_CACHE_TTL,
            max_udp_len: default_max_udp_len(),
        }
    }
    fn configure(conf: &Self::TConf) -> Self {
        Self {
            conf: *conf,
            cache: BTreeMap::new(),
            last_pruned: Instant::now(),
        }
    }
}

struct ReceivingInfo {
    expire_on: Instant,
    max_kad_height: u8,
}

struct DecoderInfo {
    decoder: ExtDecoder,
    max_kad_blocks: usize,
}

type DecoderLists = BTreeMap<[u8; TRANSMISSION_INFO_SIZE], DecoderInfo>;

enum CacheStatus {
    Receiving(ReceivingInfo, DecoderLists),
    Processed(Instant),
}

impl CacheStatus {
    fn receiving(&self) -> bool {
        matches!(&self, CacheStatus::Receiving(..))
    }

    fn expired(&self) -> bool {
        let expire_on = match self {
            CacheStatus::Receiving(info, _) => &info.expire_on,
            CacheStatus::Processed(expire_on) => expire_on,
        };
        expire_on < &Instant::now()
    }
}

impl Decoder for RaptorQDecoder {
    fn decode(&mut self, message: Message) -> io::Result<Option<Message>> {
        if let Message::Broadcast(header, payload, ..) = message {
            trace!("> Decoding broadcast chunk");
            let chunked = ChunkedPayload::try_from(&payload)?;
            let ray_id = chunked.ray_id();
            let encode_info = chunked.transmission_info_bytes();

            // Perform a `match` on the cache entry against the ray id.
            let status = match self.cache.entry(ray_id) {
                // Cache status exists: return it
                std::collections::btree_map::Entry::Occupied(o) => o.into_mut(),

                // Cache status not found: creates a new entry with
                // CacheStatus::Receiving status and binds a new Decoder with
                // the received transmission information
                std::collections::btree_map::Entry::Vacant(v) => {
                    let receiving_info = ReceivingInfo {
                        expire_on: Instant::now() + self.conf.cache_ttl,
                        max_kad_height: payload.height,
                    };
                    v.insert(CacheStatus::Receiving(
                        receiving_info,
                        BTreeMap::new(),
                    ))
                }
            };

            let decoded = match status {
                // Avoid to repropagate already processed messages
                CacheStatus::Processed(_) => None,
                CacheStatus::Receiving(recv, list) => {
                    // check right decoder according to the encoding info
                    let decoder_info = match list.entry(encode_info) {
                        // Cache status exists: return it
                        std::collections::btree_map::Entry::Occupied(o) => {
                            o.into_mut()
                        }

                        // Cache status not found: creates a new entry with
                        // CacheStatus::Receiving status and binds a new Decoder
                        // with the received
                        // transmission information
                        std::collections::btree_map::Entry::Vacant(v) => {
                            let info = chunked
                                .transmission_info(self.conf.max_udp_len);

                            match info {
                                Ok(safe_info) => {
                                    debug!(
                                        event = "Start decoding payload",
                                        ray = hex::encode(ray_id),
                                        encode_info = hex::encode(encode_info)
                                    );

                                    v.insert(DecoderInfo {
                                        decoder: ExtDecoder::new(
                                            safe_info.inner,
                                        ),
                                        max_kad_blocks: safe_info.max_blocks,
                                    })
                                }
                                Err(e) => {
                                    return Err(io::Error::other(
                                        format!(
                                            "Invalid transmission info {e:?}",
                                        ),
                                    ));
                                }
                            }
                        }
                    };

                    // Depending on Beta replication, we can receive chunks of
                    // the same message from multiple peers.
                    // Those peers can send with different broadcast height.
                    // If those heights differs, we should check the highest one
                    // in order to preserve the propagation
                    if payload.height > recv.max_kad_height {
                        recv.max_kad_height = payload.height;
                    }

                    let packet =
                        EncodingPacket::deserialize(chunked.encoded_chunk());
                    if packet.payload_id().source_block_number() as usize
                        >= decoder_info.max_kad_blocks
                    {
                        return Ok(None);
                    };

                    decoder_info
                        .decoder
                        .decode(packet)
                        // If decoded successfully, create the new
                        // BroadcastMessage
                        .and_then(|decoded| {
                            let payload = BroadcastPayload {
                                height: recv.max_kad_height,
                                gossip_frame: decoded,
                            };
                            // Perform integrity check
                            match payload.generate_ray_id() {
                                // Compare received ID with the one generated
                                Ok(ray_id) if chunked.ray_id().eq(&ray_id) => {
                                    Some(Message::Broadcast(
                                        header, payload, ray_id,
                                    ))
                                }
                                _ => {
                                    warn!("Invalid message decoded");
                                    None
                                }
                            }
                        })
                        // If the message is succesfully decoded, update the
                        // cache with new status. This
                        // will drop useless Decoder and avoid
                        // to propagate already processed messages
                        .inspect(|_decoded| {
                            self.cache.insert(
                                ray_id,
                                CacheStatus::Processed(
                                    Instant::now() + self.conf.cache_ttl,
                                ),
                            );
                            trace!("> Broadcast message decoded!");
                        })
                }
            };
            // Every X time, prune dupemap cache
            if self.last_pruned.elapsed() > self.conf.cache_prune_every {
                self.cache.retain(|ray_id, status| {
                    let keep = !status.expired();
                    if !keep && status.receiving() {
                        warn!(
                            event = "dupemap discard",
                            ray = hex::encode(ray_id)
                        );
                    };
                    keep
                });
                self.last_pruned = Instant::now();
            }
            Ok(decoded)
        } else {
            Ok(Some(message))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;
    use crate::peer::PeerNode;
    use crate::tests::Result;
    use crate::transport::encoding::raptorq::RaptorQEncoder;
    use crate::transport::encoding::Encoder;

    impl RaptorQDecoder {
        fn cache_size(&self) -> usize {
            self.cache.len()
        }
    }

    #[test]
    fn test_expiring_cache() -> Result<()> {
        let root = PeerNode::generate("192.168.0.1:666", 0)?;
        let enc =
            RaptorQEncoder::configure(&RaptorQEncoder::default_configuration());
        let mut conf = RaptorQDecoder::default_configuration();
        conf.cache_prune_every = Duration::from_millis(500);
        conf.cache_ttl = Duration::from_secs(1);
        let mut dec = RaptorQDecoder::configure(&conf);
        assert_eq!(dec.cache_size(), 0);

        //Decode first message
        for n in enc.encode(Message::broadcast(
            root.to_header(),
            BroadcastPayload {
                height: 0,
                gossip_frame: vec![0],
            },
        ))? {
            dec.decode(n)?;
        }
        assert_eq!(dec.cache_size(), 1);

        // Wait for first check, message should not expire
        thread::sleep(Duration::from_millis(500));
        assert_eq!(dec.cache_size(), 1);

        // Wait second check, next decode should remove first message
        thread::sleep(Duration::from_millis(500));

        // Decode other 3 messages
        for i in 1..4 {
            for n in enc.encode(Message::broadcast(
                root.to_header(),
                BroadcastPayload {
                    height: 0,
                    gossip_frame: vec![i],
                },
            ))? {
                dec.decode(n)?;
            }
        }
        assert_eq!(dec.cache_size(), 3);
        thread::sleep(Duration::from_millis(500));
        assert_eq!(dec.cache_size(), 3);
        thread::sleep(Duration::from_millis(500));

        // Decode message, it should remove the previous 3
        for n in enc.encode(Message::broadcast(
            root.to_header(),
            BroadcastPayload {
                height: 0,
                gossip_frame: vec![0],
            },
        ))? {
            dec.decode(n)?;
        }
        assert_eq!(dec.cache_size(), 1);
        Ok(())
    }
}

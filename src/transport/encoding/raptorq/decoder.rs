// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::collections::HashMap;
use std::convert::TryInto;
use std::io;
use std::time::{Duration, Instant};

use raptorq::{Decoder as ExtDecoder, EncodingPacket};
use serde::{Deserialize, Serialize};
use tracing::{trace, warn};

use super::ChunkedPayload;
use crate::encoding::message::Message;
use crate::encoding::payload::BroadcastPayload;
use crate::transport::encoding::Configurable;
use crate::transport::Decoder;

const DEFAULT_CACHE_TTL_SECS: u64 = 60;
const DEFAULT_CACHE_PRUNE_EVERY_SECS: u64 = 60 * 5;

pub struct RaptorQDecoder {
    cache: HashMap<[u8; 32], CacheStatus>,
    last_pruned: Instant,
    conf: RaptorQDecoderConf,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct RaptorQDecoderConf {
    #[serde(with = "humantime_serde")]
    pub cache_ttl: Duration,
    #[serde(with = "humantime_serde")]
    pub cache_prune_every: Duration,
}

impl Configurable for RaptorQDecoder {
    type TConf = RaptorQDecoderConf;
    fn default_configuration() -> Self::TConf {
        RaptorQDecoderConf {
            cache_prune_every: Duration::from_secs(
                DEFAULT_CACHE_PRUNE_EVERY_SECS,
            ),
            cache_ttl: Duration::from_secs(DEFAULT_CACHE_TTL_SECS),
        }
    }
    fn configure(conf: &Self::TConf) -> Self {
        Self {
            conf: *conf,
            cache: HashMap::new(),
            last_pruned: Instant::now(),
        }
    }
}

enum CacheStatus {
    Receiving(ExtDecoder, Instant, u8),
    Processed(Instant),
}

impl CacheStatus {
    fn expired(&self) -> bool {
        let expire_on = match self {
            CacheStatus::Receiving(_, expire_on, _) => expire_on,
            CacheStatus::Processed(expire_on) => expire_on,
        };
        expire_on < &Instant::now()
    }
}

impl Decoder for RaptorQDecoder {
    fn decode(&mut self, message: Message) -> io::Result<Option<Message>> {
        if let Message::Broadcast(header, payload) = message {
            trace!("> Decoding broadcast chunk");
            let chunked: ChunkedPayload = (&payload).try_into()?;
            let uid = chunked.safe_uid();

            // Perform a `match` on the cache entry against the uid.
            let status = match self.cache.entry(uid) {
                // Cache status exists: return it
                std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),

                // Cache status not found: creates a new entry with
                // CacheStatus::Receiving status and binds a new Decoder with
                // the received transmission information
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(CacheStatus::Receiving(
                        ExtDecoder::new(chunked.transmission_info()),
                        Instant::now() + self.conf.cache_ttl,
                        payload.height,
                    ))
                }
            };

            let decoded = match status {
                // Avoid to repropagate already processed messages
                CacheStatus::Processed(_) => None,
                CacheStatus::Receiving(decoder, _, max_height) => {
                    // Depending on Beta replication, we can receive chunks of
                    // the same message from multiple peers.
                    // Those peers can send with different broadcast height.
                    // If those heights differs, we should check the highest one
                    // in order to preserve the propagation
                    if payload.height > *max_height {
                        *max_height = payload.height;
                    }

                    decoder
                        .decode(EncodingPacket::deserialize(
                            chunked.encoded_chunk(),
                        ))
                        // If decoded successfully, create the new
                        // BroadcastMessage
                        .and_then(|decoded| {
                            let payload = BroadcastPayload {
                                height: *max_height,
                                gossip_frame: decoded,
                            };
                            // Perform integrity check
                            match payload.generate_uid() {
                                // Compare received ID with the one generated
                                Ok(uid) if chunked.uid().eq(&uid) => {
                                    Some(Message::Broadcast(header, payload))
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
                        .map(|decoded| {
                            self.cache.insert(
                                uid,
                                CacheStatus::Processed(
                                    Instant::now() + self.conf.cache_ttl,
                                ),
                            );
                            trace!("> Broadcast message decoded!");
                            decoded
                        })
                }
            };
            // Every X time, prune dupemap cache
            if self.last_pruned.elapsed() > self.conf.cache_prune_every {
                self.cache.retain(|_, status| !status.expired());
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
        let root = PeerNode::generate("192.168.0.1:666")?;
        let enc =
            RaptorQEncoder::configure(&RaptorQEncoder::default_configuration());
        let mut conf = RaptorQDecoder::default_configuration();
        conf.cache_prune_every = Duration::from_millis(500);
        conf.cache_ttl = Duration::from_secs(1);
        let mut dec = RaptorQDecoder::configure(&conf);
        assert_eq!(dec.cache_size(), 0);

        //Decode first message
        for n in enc.encode(Message::Broadcast(
            root.as_header(),
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
            for n in enc.encode(Message::Broadcast(
                root.as_header(),
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
        for n in enc.encode(Message::Broadcast(
            root.as_header(),
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

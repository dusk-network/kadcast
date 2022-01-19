// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::transport::{
    encoding::{BaseConfigurable, Configurable},
    Decoder,
};
use raptorq::{Decoder as ExtDecoder, EncodingPacket};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tracing::{trace, warn};

use crate::encoding::{message::Message, payload::BroadcastPayload};

use super::ChunkedPayload;

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

impl BaseConfigurable for RaptorQDecoder {
    type TConf = RaptorQDecoderConf;
    fn default_configuration() -> Self::TConf {
        RaptorQDecoderConf {
            cache_prune_every: Duration::from_secs(
                DEFAULT_CACHE_PRUNE_EVERY_SECS,
            ),
            cache_ttl: Duration::from_secs(DEFAULT_CACHE_TTL_SECS),
        }
    }
}

impl Configurable for RaptorQDecoder {
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
        expire_on > &Instant::now()
    }
}

impl Decoder for RaptorQDecoder {
    fn decode(&mut self, message: Message) -> Option<Message> {
        if let Message::Broadcast(header, payload) = message {
            trace!("> Decoding broadcast chunk");
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
                            // Perform sanity check
                            match chunked.uid() == payload.generate_uid() {
                                true => {
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
            decoded
        } else {
            Some(message)
        }
    }
}

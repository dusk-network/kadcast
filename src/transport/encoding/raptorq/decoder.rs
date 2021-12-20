// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::transport::{encoding::Configurable, Decoder};
use raptorq::{Decoder as ExtDecoder, EncodingPacket};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tracing::warn;

use crate::encoding::{message::Message, payload::BroadcastPayload};

use super::ChunkedPayload;

const DEFAULT_CACHE_TTL_SECS: u64 = 60;
const DEFAULT_CACHE_PRUNE_EVERY_SECS: u64 = 60 * 5;

pub(crate) struct RaptorQDecoder {
    cache: HashMap<[u8; 32], CacheStatus>,
    last_pruned: Instant,
    cache_ttl: Duration,
    cache_prune_every: Duration,
}
impl Configurable for RaptorQDecoder {
    fn configure(conf: &HashMap<String, String>) -> Self {
        let cache_ttl_secs = conf
            .get("cache_ttl_secs")
            .map(|s| s.parse().ok())
            .flatten()
            .unwrap_or(DEFAULT_CACHE_TTL_SECS);

        let cache_prune_every_secs = conf
            .get("cache_prune_every_secs")
            .map(|s| s.parse().ok())
            .flatten()
            .unwrap_or(DEFAULT_CACHE_PRUNE_EVERY_SECS);

        Self {
            cache_ttl: Duration::from_secs(cache_ttl_secs),
            cache_prune_every: Duration::from_secs(cache_prune_every_secs),
            cache: HashMap::new(),
            last_pruned: Instant::now(),
        }
    }
    fn default_configuration() -> HashMap<String, String> {
        let mut conf = HashMap::new();
        conf.insert(
            "cache_prune_every_secs".to_string(),
            DEFAULT_CACHE_PRUNE_EVERY_SECS.to_string(),
        );
        conf.insert(
            "cache_ttl_secs".to_string(),
            DEFAULT_CACHE_TTL_SECS.to_string(),
        );
        conf
    }
}

enum CacheStatus {
    Receiving(ExtDecoder, Instant),
    Processed(Instant),
}

impl CacheStatus {
    fn expired(&self) -> bool {
        let expire_on = match self {
            CacheStatus::Receiving(_, expire_on) => expire_on,
            CacheStatus::Processed(expire_on) => expire_on,
        };
        expire_on > &Instant::now()
    }
}

impl Decoder for RaptorQDecoder {
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
                    v.insert(CacheStatus::Receiving(
                        ExtDecoder::new(chunked.transmission_info()),
                        Instant::now() + self.cache_ttl,
                    ))
                }
            };

            let decoded = match status {
                // Avoid to repropagate already processed messages
                CacheStatus::Processed(_) => None,
                CacheStatus::Receiving(decoder, _) => decoder
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
                    .map(|decoded| {
                        self.cache.insert(
                            uid,
                            CacheStatus::Processed(
                                Instant::now() + self.cache_ttl,
                            ),
                        );
                        decoded
                    }),
            };
            // Every X time, prune dupemap cache
            if self.last_pruned.elapsed() > self.cache_prune_every {
                self.cache.retain(|_, status| !status.expired());
                self.last_pruned = Instant::now();
            }
            decoded
        } else {
            Some(message)
        }
    }
}

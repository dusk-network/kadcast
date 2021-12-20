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

const CACHE_DEFAULT_TTL_SECS: u64 = 60;
const CACHE_PRUNED_EVERY_SECS: u64 = 60 * 5;

const CACHE_DEFAULT_TTL_DURATION: Duration =
    Duration::from_secs(CACHE_DEFAULT_TTL_SECS);
const CACHE_PRUNED_EVERY_DURATION: Duration =
    Duration::from_secs(CACHE_PRUNED_EVERY_SECS);

pub(crate) struct RaptorQDecoder {
    cache: HashMap<[u8; 32], CacheStatus>,
    last_pruned: Instant,
}
impl Configurable for RaptorQDecoder {
    fn configure(_: HashMap<String, String>) -> Self {
        Self {
            cache: HashMap::new(),
            last_pruned: Instant::now(),
        }
    }
}

enum CacheStatus {
    Receiving(ExtDecoder, Instant),
    Processed(Instant),
}

impl CacheStatus {
    fn expired(&self) -> bool {
        match self {
            CacheStatus::Receiving(_, date) => date,
            CacheStatus::Processed(date) => date,
        }
        .elapsed()
            > CACHE_DEFAULT_TTL_DURATION
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
                        Instant::now(),
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
                            CacheStatus::Processed(Instant::now()),
                        );
                        decoded
                    }),
            };
            // Every X time, prune dupemap cache
            if self.last_pruned.elapsed() > CACHE_PRUNED_EVERY_DURATION {
                self.cache.retain(|_, status| !status.expired());
                self.last_pruned = Instant::now();
            }
            decoded
        } else {
            Some(message)
        }
    }
}

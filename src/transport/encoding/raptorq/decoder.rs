// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::convert::TryFrom;
use std::io;
use std::net::{IpAddr, Ipv6Addr};
use std::time::{Duration, Instant};

use raptorq::{Decoder as ExtDecoder, EncodingPacket};
use serde::{Deserialize, Serialize};
use tracing::{debug, trace, warn};

use super::safe::SafeObjectTransmissionInformation;
use super::{ChunkedPayload, RAY_ID_SIZE, TRANSMISSION_INFO_SIZE};
use crate::encoding::message::Message;
use crate::encoding::payload::BroadcastPayload;
use crate::transport::Decoder;
use crate::transport::encoding::Configurable;

type RayId = [u8; RAY_ID_SIZE];
type DecoderKey = (RayId, [u8; TRANSMISSION_INFO_SIZE]);

// Fixed TTLs keep insertion and expiry order identical. Entries are removed
// only from the front, so neither queue can accumulate stale records.
#[derive(Default)]
struct RayCache<T> {
    entries: BTreeMap<RayId, (Instant, T)>,
    expiry: VecDeque<(Instant, RayId)>,
}

impl<T> RayCache<T> {
    fn evict_oldest(&mut self) {
        if let Some((_, ray)) = self.expiry.pop_front() {
            self.entries.remove(&ray);
        }
    }

    fn prune(&mut self, now: Instant) {
        while self
            .expiry
            .front()
            .is_some_and(|(expiry, _)| *expiry <= now)
        {
            self.evict_oldest();
        }
    }

    fn insert(&mut self, ray: RayId, expiry: Instant, value: T, limit: usize) {
        if limit == 0 {
            return;
        }
        debug_assert!(!self.entries.contains_key(&ray));
        while self.entries.len() >= limit {
            self.evict_oldest();
        }
        self.entries.insert(ray, (expiry, value));
        self.expiry.push_back((expiry, ray));
    }
}

#[derive(Default)]
struct SourceUsage {
    bytes: u64,
    count: usize,
}

pub struct RaptorQDecoder {
    receiving: BTreeMap<DecoderKey, Receiving>,
    processed: RayCache<()>,
    heights: RayCache<u8>,
    cached_bytes: u64,
    source_usage: BTreeMap<IpAddr, SourceUsage>,
    last_pruned: Instant,
    conf: RaptorQDecoderConf,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(default)]
pub struct RaptorQDecoderConf {
    #[serde(with = "humantime_serde")]
    pub cache_ttl: Duration,
    #[serde(with = "humantime_serde")]
    pub cache_prune_every: Duration,
    /// Maximum transfer length (exclusive), in bytes. Both memory budgets
    /// also apply: raising this alone does not guarantee admission. With the
    /// default budgets, transfers above roughly 30 MiB are not cached; raise
    /// the budgets as well to accept larger transfers.
    pub max_udp_len: u64,
    /// Maximum completed ray IDs and ray-height records remembered (each).
    pub max_cache_entries: usize,
    /// Maximum incomplete decoders, across all rays and configurations.
    pub max_decoders: usize,
    /// Per source (an IPv4 address or an IPv6 /64), including all ports and
    /// transmission configurations.
    pub max_decoders_per_source: usize,
    /// Estimated memory budget for incomplete transfers, in bytes, charged as
    /// packets arrive. Includes stored symbols, decoded blocks, duplicate
    /// tracking and allocation slack; excludes temporary RaptorQ solver
    /// allocations and transport queues. Transfers whose worst case exceeds
    /// this budget or the per-source share are not cached.
    pub max_cache_bytes: u64,
    /// Per-source share of the reassembly memory budget, in bytes.
    pub max_cache_bytes_per_source: u64,
}

impl Default for RaptorQDecoderConf {
    fn default() -> Self {
        Self {
            cache_ttl: Duration::from_secs(60),
            cache_prune_every: Duration::from_secs(30),
            max_udp_len: 10 * 1_024 * 1_024,
            max_cache_entries: 4_096,
            max_decoders: 128,
            max_decoders_per_source: 8,
            max_cache_bytes: 256 * 1_024 * 1_024,
            max_cache_bytes_per_source: 128 * 1_024 * 1_024,
        }
    }
}

impl Configurable for RaptorQDecoder {
    type TConf = RaptorQDecoderConf;

    fn default_configuration() -> Self::TConf {
        RaptorQDecoderConf::default()
    }

    fn configure(conf: &Self::TConf) -> Self {
        Self {
            conf: *conf,
            receiving: BTreeMap::new(),
            processed: RayCache::default(),
            heights: RayCache::default(),
            cached_bytes: 0,
            source_usage: BTreeMap::new(),
            last_pruned: Instant::now(),
        }
    }
}

struct Receiving {
    // The source that starts this decoder is charged for it. Compatible
    // chunks from other peers still contribute. Ports/IDs are not quotas.
    source: IpAddr,
    expire_on: Instant,
    last_packet: Instant,
    max_kad_height: u8,
    decoder: ExtDecoder,
    // Estimated retained bytes, charged as packets are accepted.
    bytes: u64,
    packet_bytes: u64,
    // Per-block allowance: one block cannot consume other blocks' repair
    // space.
    packet_limit: usize,
    packets_per_block: Vec<usize>,
    packets: BTreeSet<(u8, u32)>,
}

impl Receiving {
    /// Returns `None` if the transfer could outgrow the memory budget.
    fn new(
        info: &SafeObjectTransmissionInformation,
        source: IpAddr,
        now: Instant,
        conf: &RaptorQDecoderConf,
    ) -> Option<Self> {
        let symbol_size = u64::from(info.inner.symbol_size());
        let blocks = info.max_blocks as u64;
        let symbols = info.inner.transfer_length().div_ceil(symbol_size);
        // A finite repair allowance is essential: the external decoder
        // retains distinct repair symbols even after failed solve attempts.
        let packet_limit = symbols.div_ceil(blocks) * 2 + 16;
        // Each packet keeps its symbol and, once its block decodes, at most one
        // symbol of output, plus collection and allocator overhead.
        let packet_bytes = symbol_size * 2 + 128;
        // Symbol slots and per-block state are allocated up front.
        let bytes = symbols * 32 + blocks * 512;
        let max_bytes = bytes + packet_limit * blocks * packet_bytes;
        if max_bytes > conf.max_cache_bytes.min(conf.max_cache_bytes_per_source)
        {
            return None;
        }
        Some(Self {
            source,
            expire_on: now + conf.cache_ttl,
            last_packet: now,
            max_kad_height: 0,
            decoder: ExtDecoder::new(info.inner),
            bytes,
            packet_bytes,
            packet_limit: packet_limit as usize,
            packets_per_block: vec![0; info.max_blocks],
            packets: BTreeSet::new(),
        })
    }
}

/// Group IPv6 sources by /64; mapped IPv4 addresses keep their IPv4 quota.
fn source_of(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V6(ip) => {
            Ipv6Addr::from(u128::from(ip) & !(u128::MAX >> 64)).into()
        }
        ip => ip,
    }
}

impl RaptorQDecoder {
    // All removals debit the same totals, including eviction and completion.
    fn take(&mut self, key: &DecoderKey) -> Option<Receiving> {
        let recv = self.receiving.remove(key)?;
        self.cached_bytes -= recv.bytes;
        if let Some(usage) = self.source_usage.get_mut(&recv.source) {
            usage.bytes -= recv.bytes;
            usage.count -= 1;
            if usage.count == 0 {
                self.source_usage.remove(&recv.source);
            }
        }
        Some(recv)
    }

    fn prune(&mut self, now: Instant) {
        let expired: Vec<_> = self
            .receiving
            .iter()
            .filter(|(_, recv)| recv.expire_on <= now)
            .map(|(key, _)| *key)
            .collect();
        for key in expired {
            self.take(&key);
            warn!(event = "dupemap discard", ray = hex::encode(key.0));
        }
        self.last_pruned = now;
    }

    /// Caches an incomplete transfer, evicting others if needed. The transfer
    /// must not be in the cache, so it can never evict itself.
    fn store(&mut self, key: DecoderKey, recv: Receiving, now: Instant) {
        if self.make_room(recv.source, recv.bytes, now) {
            self.cached_bytes += recv.bytes;
            let usage = self.source_usage.entry(recv.source).or_default();
            usage.bytes += recv.bytes;
            usage.count += 1;
            self.receiving.insert(key, recv);
        } else {
            trace!("Dropped incomplete RaptorQ transfer: no room");
        }
    }

    fn make_room(&mut self, source: IpAddr, bytes: u64, now: Instant) -> bool {
        let conf = self.conf;
        if conf.max_decoders == 0
            || conf.max_decoders_per_source == 0
            || bytes > conf.max_cache_bytes
            || bytes > conf.max_cache_bytes_per_source
        {
            return false;
        }
        loop {
            let over_source =
                self.source_usage.get(&source).is_some_and(|usage| {
                    usage.count >= conf.max_decoders_per_source
                        || bytes
                            > conf
                                .max_cache_bytes_per_source
                                .saturating_sub(usage.bytes)
                });
            if !over_source
                && self.receiving.len() < conf.max_decoders
                && bytes
                    <= conf.max_cache_bytes.saturating_sub(self.cached_bytes)
            {
                return true;
            }
            // Normal packets use running totals; only pressure scans for a
            // victim, preferring expired then least recently active transfers.
            let victim = self
                .receiving
                .iter()
                .filter(|(_, recv)| !over_source || recv.source == source)
                .min_by_key(|(_, recv)| {
                    (recv.expire_on > now, recv.last_packet)
                })
                .map(|(key, _)| *key);
            let Some(victim) = victim else {
                return false;
            };
            if let Some(recv) = self.take(&victim) {
                let ray = hex::encode(victim.0);
                if recv.expire_on > now {
                    debug!(event = "dupemap evict", ray);
                } else {
                    warn!(event = "dupemap discard", ray);
                }
            }
        }
    }
}

impl Decoder for RaptorQDecoder {
    fn decode(
        &mut self,
        message: Message,
        source: IpAddr,
    ) -> io::Result<Option<Message>> {
        let Message::Broadcast(header, payload, ..) = message else {
            return Ok(Some(message));
        };
        let source = source_of(source);
        let now = Instant::now();
        self.processed.prune(now);
        self.heights.prune(now);
        if now.duration_since(self.last_pruned) >= self.conf.cache_prune_every {
            self.prune(now);
        }
        let chunked = ChunkedPayload::try_from(&payload)?;
        let ray_id = chunked.ray_id();
        if self.processed.entries.contains_key(&ray_id) {
            return Ok(None);
        }
        // Validate everything before inserting cache entries or constructing an
        // external decoder. Malformed traffic must not grow either cache.
        let info =
            chunked
                .transmission_info(self.conf.max_udp_len)
                .map_err(|e| {
                    io::Error::other(format!("Invalid transmission info {e:?}"))
                })?;
        let encoded = chunked.encoded_chunk();
        if encoded.len() != 4 + usize::from(info.inner.symbol_size())
            || usize::from(encoded[0]) >= info.max_blocks
        {
            return Err(io::Error::other(
                "Invalid RaptorQ symbol length or block",
            ));
        }
        // Take the transfer out of the cache while processing it. It is stored
        // again only if still incomplete, so messages completed by their first
        // packet never take a slot or evict anything.
        let key = (ray_id, chunked.transmission_info_bytes());
        let mut recv = match self.take(&key) {
            Some(recv) if recv.expire_on > now => recv,
            _ => match Receiving::new(&info, source, now, &self.conf) {
                Some(recv) => {
                    debug!(
                        event = "Start decoding payload",
                        ray = hex::encode(key.0),
                        encode_info = hex::encode(key.1)
                    );
                    recv
                }
                None => {
                    trace!("Dropped RaptorQ transfer exceeding memory budget");
                    return Ok(None);
                }
            },
        };
        recv.max_kad_height = recv.max_kad_height.max(payload.height);
        if let Some((_, height)) = self.heights.entries.get_mut(&ray_id) {
            *height = (*height).max(recv.max_kad_height);
            recv.max_kad_height = *height;
        } else {
            self.heights.insert(
                ray_id,
                now + self.conf.cache_ttl,
                recv.max_kad_height,
                self.conf.max_cache_entries,
            );
        }
        let packet = EncodingPacket::deserialize(encoded);
        let packet_id = (
            packet.payload_id().source_block_number(),
            packet.payload_id().encoding_symbol_id(),
        );
        if recv.packets.contains(&packet_id) {
            self.store(key, recv, now);
            return Ok(None);
        }
        let block = usize::from(packet_id.0);
        if recv.packets_per_block[block] >= recv.packet_limit {
            trace!("Dropped RaptorQ packet exceeding repair allowance");
            self.store(key, recv, now);
            return Ok(None);
        }
        recv.packets.insert(packet_id);
        recv.packets_per_block[block] += 1;
        recv.bytes += recv.packet_bytes;
        recv.last_packet = now;
        let Some(decoded) = recv.decoder.decode(packet) else {
            self.store(key, recv, now);
            return Ok(None);
        };
        let mut payload = BroadcastPayload {
            height: recv.max_kad_height,
            gossip_frame: decoded,
        };
        if payload.generate_ray_id()? != ray_id {
            warn!("Invalid message decoded");
            return Ok(None);
        }
        // Completion releases all source/configuration variants for this ray.
        let variants: Vec<_> = self
            .receiving
            .range(
                (ray_id, [0; TRANSMISSION_INFO_SIZE])
                    ..=(ray_id, [u8::MAX; TRANSMISSION_INFO_SIZE]),
            )
            .map(|(key, _)| *key)
            .collect();
        for key in variants {
            if let Some(variant) = self.take(&key) {
                payload.height = payload.height.max(variant.max_kad_height);
            }
        }
        self.processed.insert(
            ray_id,
            now + self.conf.cache_ttl,
            (),
            self.conf.max_cache_entries,
        );
        trace!("> Broadcast message decoded!");
        Ok(Some(Message::Broadcast(header, payload, ray_id)))
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use super::super::CHUNKED_HEADER_SIZE;
    use super::super::encoder::MIN_MTU;
    use super::*;
    use crate::encoding::message::Header;
    use crate::peer::PeerNode;
    use crate::tests::Result;
    use crate::transport::encoding::Encoder;
    use crate::transport::encoding::raptorq::RaptorQEncoder;

    impl RaptorQDecoder {
        fn cache_size(&self) -> usize {
            self.processed.entries.len() + self.receiving.len()
        }

        fn rays(&self) -> Vec<u8> {
            self.receiving.keys().map(|key| key.0[0]).collect()
        }

        fn cached_bytes(&self) -> u64 {
            let total: u64 =
                self.receiving.values().map(|recv| recv.bytes).sum();
            assert_eq!(self.cached_bytes, total);
            assert_eq!(
                self.source_usage
                    .values()
                    .map(|usage| usage.bytes)
                    .sum::<u64>(),
                total
            );
            assert_eq!(
                self.source_usage
                    .values()
                    .map(|usage| usage.count)
                    .sum::<usize>(),
                self.receiving.len()
            );
            for (source, usage) in &self.source_usage {
                let entries: Vec<_> = self
                    .receiving
                    .values()
                    .filter(|recv| recv.source == *source)
                    .collect();
                assert_eq!(usage.count, entries.len());
                assert_eq!(
                    usage.bytes,
                    entries.iter().map(|recv| recv.bytes).sum::<u64>()
                );
            }
            total
        }
    }

    fn chunk(
        header: Header,
        ray: u16,
        info: raptorq::ObjectTransmissionInformation,
        block: u8,
        esi: u32,
    ) -> Message {
        let mut id = [0; RAY_ID_SIZE];
        id[..2].copy_from_slice(&ray.to_le_bytes());
        let packet = EncodingPacket::new(
            raptorq::PayloadId::new(block, esi),
            vec![0; usize::from(info.symbol_size())],
        );
        Message::broadcast(
            header,
            BroadcastPayload {
                height: 0,
                gossip_frame: [
                    id.to_vec(),
                    info.serialize().to_vec(),
                    packet.serialize(),
                ]
                .concat(),
            },
        )
    }

    #[test]
    fn malformed_chunks_do_not_allocate_state() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let info =
            raptorq::ObjectTransmissionInformation::with_defaults(20_000, 1300);
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf {
            max_udp_len: u64::MAX,
            ..RaptorQDecoderConf::default()
        });
        // OTI offsets: length=0..5, symbol size=6..8, blocks=8,
        // sub-blocks=9..11, alignment=11. All mutations are wire-reachable.
        for (offset, value) in
            [(6, 0), (7, 0), (7, 17), (8, 0), (10, 0), (9, 255), (11, 0)]
        {
            let mut msg = chunk(root.to_header(), 0, info, 0, 0);
            let Message::Broadcast(_, ref mut payload, _) = msg else {
                unreachable!()
            };
            if offset == 6 || offset == 7 && value == 0 {
                payload.gossip_frame[RAY_ID_SIZE + 6] = 0;
                payload.gossip_frame[RAY_ID_SIZE + 7] = 0;
            } else {
                payload.gossip_frame[RAY_ID_SIZE + offset] = value;
            }
            assert!(dec.decode(msg, root.value().address().ip()).is_err());
            assert!(dec.receiving.is_empty());
        }
        for kind in 0..5 {
            let mut msg = chunk(root.to_header(), 0, info, 0, 0);
            let Message::Broadcast(_, ref mut payload, _) = msg else {
                unreachable!()
            };
            match kind {
                0 => {
                    payload.gossip_frame.pop();
                }
                1 => payload.gossip_frame.push(0),
                2 => payload.gossip_frame[CHUNKED_HEADER_SIZE] = 255,
                3 => {
                    payload.gossip_frame[RAY_ID_SIZE..RAY_ID_SIZE + 5].fill(255)
                }
                _ => {
                    payload.gossip_frame[RAY_ID_SIZE..RAY_ID_SIZE + 4].fill(0);
                    payload.gossip_frame[RAY_ID_SIZE + 4] = 1;
                    payload.gossip_frame[RAY_ID_SIZE + 8] = 2;
                }
            }
            assert!(dec.decode(msg, root.value().address().ip()).is_err());
            assert!(dec.receiving.is_empty());
            assert!(dec.processed.entries.is_empty());
        }
    }

    #[test]
    fn bound_solver_dimensions_without_reducing_default_transfer_limit() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let source = root.value().address().ip();
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf::default());
        // Tiny symbols would turn a small transfer into a large solver.
        let oversized_solver =
            raptorq::ObjectTransmissionInformation::new(8 * 8_193, 8, 1, 1, 8);
        assert!(
            dec.decode(
                chunk(root.to_header(), 0, oversized_solver, 0, 0),
                source
            )
            .is_err()
        );
        assert!(dec.receiving.is_empty());
        let maximum_transfer =
            raptorq::ObjectTransmissionInformation::with_defaults(
                dec.conf.max_udp_len - 1,
                MIN_MTU,
            );
        dec.decode(chunk(root.to_header(), 1, maximum_transfer, 0, 0), source)
            .unwrap();
        assert_eq!(dec.receiving.len(), 1);
        // One packet is charged for symbol slots and itself, not the worst
        // case.
        assert!(dec.cached_bytes() < 1_024 * 1_024);
    }

    #[test]
    fn distinct_rays_and_configurations_stay_bounded() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf {
            max_decoders: 4,
            max_decoders_per_source: 2,
            ..RaptorQDecoderConf::default()
        });
        for i in 0..1_000u16 {
            let source = IpAddr::from([192, 0, 2, (i / 100 + 1) as u8]);
            let info = raptorq::ObjectTransmissionInformation::with_defaults(
                20_000,
                1296 + i % 100 * 8,
            );
            let mut header = root.to_header();
            header.sender_port = i;
            // Alternate distinct rays with many OTI variants for one ray.
            dec.decode(
                chunk(header, if i % 2 == 0 { i } else { 0 }, info, 0, 0),
                source,
            )
            .unwrap();
            assert!(dec.receiving.len() <= 4);
            assert!(
                dec.receiving
                    .values()
                    .filter(|r| r.source == source)
                    .count()
                    <= 2
            );
        }
        assert_eq!(dec.receiving.len(), 4);
    }

    #[test]
    fn pressure_evicts_least_recently_active_and_respects_source_share() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let [a, b, c] = [1, 2, 3].map(|i| IpAddr::from([192, 0, 2, i]));
        let info =
            raptorq::ObjectTransmissionInformation::with_defaults(20_000, 1296);
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf {
            max_decoders: 3,
            max_decoders_per_source: 2,
            ..RaptorQDecoderConf::default()
        });
        let send = |dec: &mut RaptorQDecoder, ray, esi, source| {
            let msg = chunk(root.to_header(), ray, info, 0, esi);
            assert!(dec.decode(msg, source).unwrap().is_none());
        };
        send(&mut dec, 1, 0, a);
        send(&mut dec, 2, 0, a);
        send(&mut dec, 3, 0, b);
        // Activity order differs from creation order: 3, 2, 1.
        send(&mut dec, 2, 1, a);
        send(&mut dec, 1, 1, a);
        // A third ray from `a` exceeds its share: `a` loses its idlest ray.
        send(&mut dec, 4, 0, a);
        assert_eq!(dec.rays(), [1, 3, 4]);
        // Global pressure evicts the idlest ray overall, not the oldest.
        send(&mut dec, 5, 0, c);
        assert_eq!(dec.rays(), [1, 4, 5]);
        // Expired transfers go first, even if recently active.
        let ray5 = dec.receiving.values_mut().find(|r| r.source == c);
        ray5.unwrap().expire_on = Instant::now();
        send(&mut dec, 6, 0, b);
        assert_eq!(dec.rays(), [1, 4, 6]);
        // A transfer that could outgrow the budget must not evict anything.
        dec.conf.max_cache_bytes_per_source = 1_024;
        send(&mut dec, 7, 0, c);
        assert_eq!(dec.rays(), [1, 4, 6]);
    }

    #[test]
    fn byte_budget_tracks_received_packets() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let [a, b, c] = [1, 2, 3].map(|i| IpAddr::from([192, 0, 2, i]));
        // 155 source symbols: up to 154 packets keep a transfer incomplete.
        let info = raptorq::ObjectTransmissionInformation::with_defaults(
            200_000, 1296,
        );
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf::default());
        dec.decode(chunk(root.to_header(), 0, info, 0, 0), a)
            .unwrap();
        let recv = dec.receiving.values().next().unwrap();
        // The budget fits exactly one transfer's worst case.
        let budget =
            recv.bytes + (recv.packet_limit as u64 - 1) * recv.packet_bytes;
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf {
            max_cache_bytes: budget,
            ..RaptorQDecoderConf::default()
        });
        for (ray, source) in [(1, a), (2, b), (3, c)] {
            for esi in 0..120 {
                let msg = chunk(root.to_header(), ray, info, 0, esi);
                assert!(dec.decode(msg, source).unwrap().is_none());
                assert!(dec.cached_bytes() <= budget);
            }
        }
        // Three partial transfers exceed the budget: the idlest one went.
        assert_eq!(dec.rays(), [2, 3]);
    }

    #[test]
    fn source_byte_budget_evicts_only_that_sources_idlest_transfer() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let a = root.value().address().ip();
        let b = IpAddr::from([192, 0, 2, 2]);
        // Withhold block one, so block zero can reach its allowance without
        // completing the ray. Two partial transfers can exceed one worst case.
        let info = raptorq::ObjectTransmissionInformation::new(
            200_000, MIN_MTU, 2, 1, 8,
        );
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf::default());
        dec.decode(chunk(root.to_header(), 0, info, 0, 0), a)
            .unwrap();
        let recv = dec.receiving.values().next().unwrap();
        let packets =
            recv.packet_limit as u64 * u64::from(info.source_blocks());
        let budget = recv.bytes + (packets - 1) * recv.packet_bytes;
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf {
            max_cache_bytes_per_source: budget,
            ..RaptorQDecoderConf::default()
        });
        let send = |dec: &mut RaptorQDecoder, ray, esi, source| {
            assert!(
                dec.decode(chunk(root.to_header(), ray, info, 0, esi), source)
                    .unwrap()
                    .is_none()
            );
            dec.cached_bytes(); // cross-check every accounting path
        };
        // The other source is oldest overall and must not be evicted.
        send(&mut dec, 3, 0, b);
        for ray in [1, 2] {
            for esi in 0..171 {
                send(&mut dec, ray, esi, a);
            }
        }
        assert_eq!(dec.rays(), [2, 3]);
        assert!(dec.source_usage[&a].bytes <= budget);
        assert_eq!(dec.source_usage[&b].count, 1);
        assert!(dec.cached_bytes() < dec.conf.max_cache_bytes);
    }

    #[test]
    fn ipv6_sources_are_grouped_by_prefix() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let info = raptorq::ObjectTransmissionInformation::with_defaults(
            20_000, MIN_MTU,
        );
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf {
            max_decoders_per_source: 1,
            ..RaptorQDecoderConf::default()
        });
        for (ray, ip) in [
            (1, "2001:db8::1"),
            (2, "2001:db8::ffff:1"),
            (3, "2001:db8:0:1::1"),
            (4, "192.0.2.1"),
            (5, "::ffff:192.0.2.1"),
            (6, "::ffff:192.0.2.2"),
        ] {
            dec.decode(
                chunk(root.to_header(), ray, info, 0, 0),
                ip.parse().unwrap(),
            )
            .unwrap();
        }
        assert_eq!(dec.rays(), [2, 3, 5, 6]);
        dec.cached_bytes();
    }

    #[test]
    fn active_transfer_survives_large_announcements() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let enc =
            RaptorQEncoder::configure(&RaptorQEncoder::default_configuration());
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf::default());
        let chunks = enc
            .encode(Message::broadcast(
                root.to_header(),
                BroadcastPayload {
                    height: 0,
                    gossip_frame: vec![7; 1_000_000],
                },
            ))
            .unwrap();
        let half = chunks.len() / 2;
        let mut chunks = chunks.into_iter();
        let legit = IpAddr::from([192, 0, 2, 1]);
        for msg in chunks.by_ref().take(half) {
            assert!(dec.decode(msg, legit).unwrap().is_none());
        }
        // Fill every other slot with single packets announcing maximum-size
        // transfers. Each is charged for what it sent, not its worst case.
        let announced = raptorq::ObjectTransmissionInformation::with_defaults(
            dec.conf.max_udp_len - 1,
            MIN_MTU,
        );
        for i in 1..dec.conf.max_decoders as u16 {
            let source = IpAddr::from([198, 51, 100, (i % 16) as u8]);
            let msg = chunk(root.to_header(), i, announced, 0, 0);
            assert!(dec.decode(msg, source).unwrap().is_none());
        }
        assert_eq!(dec.receiving.len(), dec.conf.max_decoders);
        assert!(dec.cached_bytes() < dec.conf.max_cache_bytes / 4);
        let decoded = chunks
            .filter_map(|msg| dec.decode(msg, legit).unwrap())
            .count();
        assert_eq!(decoded, 1);
    }

    #[test]
    fn junk_traffic_cannot_starve_new_broadcasts() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let enc =
            RaptorQEncoder::configure(&RaptorQEncoder::default_configuration());
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf::default());
        // Single-symbol junk rays decode immediately and fail the ray check.
        let info =
            raptorq::ObjectTransmissionInformation::with_defaults(1, MIN_MTU);
        for i in 0..10_000u16 {
            let source = IpAddr::from([198, 51, 100, (i % 8) as u8]);
            let junk = chunk(root.to_header(), i, info, 0, 0);
            assert!(dec.decode(junk, source).unwrap().is_none());
        }
        // Rays completed by their first packet never occupy a decoder.
        assert!(dec.receiving.is_empty());
        let legit = enc
            .encode(Message::broadcast(
                root.to_header(),
                BroadcastPayload {
                    height: 0,
                    gossip_frame: vec![1, 2, 3],
                },
            ))
            .unwrap();
        let source = IpAddr::from([192, 0, 2, 9]);
        let decoded = legit
            .into_iter()
            .filter_map(|msg| dec.decode(msg, source).unwrap())
            .count();
        assert_eq!(decoded, 1);
    }

    #[test]
    fn duplicate_and_excess_symbols_cannot_grow_a_decoder() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let source = root.value().address().ip();
        let original = BroadcastPayload {
            height: 0,
            gossip_frame: vec![0; 1296 * 4],
        };
        let ray = original.generate_ray_id().unwrap();
        // Finish block zero while withholding block one. Later block-zero
        // packets must not discard that progress or grow bookkeeping forever.
        let info = raptorq::ObjectTransmissionInformation::new(
            1296 * 4,
            1296,
            2,
            1,
            8,
        );
        let send = |dec: &mut RaptorQDecoder, block, esi| {
            let mut msg = chunk(root.to_header(), 0, info, block, esi);
            let Message::Broadcast(_, ref mut payload, _) = msg else {
                unreachable!()
            };
            payload.gossip_frame[..RAY_ID_SIZE].copy_from_slice(&ray);
            dec.decode(msg, source).unwrap()
        };
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf::default());
        for _ in 0..100 {
            assert!(send(&mut dec, 0, 0).is_none());
        }
        assert_eq!(dec.receiving.values().next().unwrap().packets.len(), 1);
        let limit = dec.receiving.values().next().unwrap().packet_limit;
        for esi in 1..=limit {
            assert!(send(&mut dec, 0, esi as u32).is_none());
        }
        let bytes = dec.cached_bytes();
        for esi in limit + 1..limit + 100 {
            assert!(send(&mut dec, 0, esi as u32).is_none());
            assert_eq!(dec.cached_bytes(), bytes);
            assert_eq!(
                dec.receiving.values().next().unwrap().packets.len(),
                limit
            );
        }
        assert!(send(&mut dec, 1, 0).is_none());
        let Some(Message::Broadcast(_, payload, _)) = send(&mut dec, 1, 1)
        else {
            panic!("block one must finish the retained transfer")
        };
        assert_eq!(payload, original);
        assert_eq!(dec.cached_bytes(), 0);
        assert!(dec.receiving.is_empty());
        assert!(dec.source_usage.is_empty());
    }

    #[test]
    fn height_survives_eviction_with_same_or_different_configuration() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let source = root.value().address().ip();
        let enc =
            RaptorQEncoder::configure(&RaptorQEncoder::default_configuration());
        for (mtu, decoders, metadata) in
            [(1300, 1, 16), (1400, 1, 16), (1300, 3, 1), (1400, 3, 1)]
        {
            let original = vec![7; 20_000];
            let chunks = enc
                .encode(Message::broadcast(
                    root.to_header(),
                    BroadcastPayload {
                        height: 0,
                        gossip_frame: original.clone(),
                    },
                ))
                .unwrap();
            let Message::Broadcast(_, payload, _) = &chunks[0] else {
                unreachable!()
            };
            let ray = ChunkedPayload::try_from(payload).unwrap().ray_id();
            let info = raptorq::ObjectTransmissionInformation::with_defaults(
                20_000, mtu,
            );
            let mut high = chunk(root.to_header(), 0, info, 0, 0);
            let Message::Broadcast(_, ref mut payload, _) = high else {
                unreachable!()
            };
            payload.gossip_frame[..RAY_ID_SIZE].copy_from_slice(&ray);
            payload.gossip_frame[CHUNKED_HEADER_SIZE + 4..].fill(7);
            payload.height = 7;
            let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf {
                max_decoders: decoders,
                max_cache_entries: metadata,
                ..RaptorQDecoderConf::default()
            });
            assert!(dec.decode(high, source).unwrap().is_none());
            dec.decode(chunk(root.to_header(), 1, info, 0, 0), source)
                .unwrap();
            if decoders == 1 {
                // Transfer eviction retains height metadata.
                assert!(dec.receiving.keys().all(|key| key.0 != ray));
                assert!(dec.heights.entries.contains_key(&ray));
            } else {
                // Metadata eviction still permits merging surviving variants.
                assert!(!dec.heights.entries.contains_key(&ray));
                assert!(dec.receiving.keys().any(|key| key.0 == ray));
            }
            let decoded: Vec<_> = chunks
                .into_iter()
                .filter_map(|msg| dec.decode(msg, source).unwrap())
                .collect();
            assert_eq!(decoded.len(), 1);
            let Message::Broadcast(_, payload, _) = &decoded[0] else {
                unreachable!()
            };
            assert_eq!(payload.height, 7);
            assert_eq!(payload.gossip_frame, original);
            assert!(dec.receiving.keys().all(|key| key.0 != ray));
            assert_eq!(dec.receiving.len(), usize::from(decoders > 1));
            dec.cached_bytes();
        }
    }

    #[test]
    fn ray_metadata_queues_stay_bounded_and_expire_in_order() {
        let mut cache = RayCache::default();
        let now = Instant::now();
        for i in 0..10_000u16 {
            let mut ray = [0; RAY_ID_SIZE];
            ray[..2].copy_from_slice(&i.to_le_bytes());
            cache.insert(ray, now + Duration::from_secs(u64::from(i)), i, 3);
            assert!(cache.entries.len() <= 3);
            assert_eq!(cache.entries.len(), cache.expiry.len());
        }
        // Reinsert an evicted ray: no stale record can remove its new entry.
        let ray = [0; RAY_ID_SIZE];
        cache.insert(ray, now + Duration::from_secs(10_000), 0, 3);
        cache.prune(now + Duration::from_secs(9_999));
        assert_eq!(cache.entries.len(), 1);
        assert!(cache.entries.contains_key(&ray));
        cache.prune(now + Duration::from_secs(10_000));
        assert!(cache.entries.is_empty());
        assert!(cache.expiry.is_empty());
    }

    #[test]
    fn completion_combines_peers_releases_variants_and_bounds_deduplication() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let a = root.value().address().ip();
        let b = IpAddr::from([192, 0, 2, 2]);
        let enc =
            RaptorQEncoder::configure(&RaptorQEncoder::default_configuration());
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf {
            max_cache_entries: 3,
            ..RaptorQDecoderConf::default()
        });
        for i in 0..10u8 {
            let original = vec![i; 20_000];
            let chunks = enc
                .encode(Message::broadcast(
                    root.to_header(),
                    BroadcastPayload {
                        height: 2,
                        gossip_frame: original.clone(),
                    },
                ))
                .unwrap();
            let Message::Broadcast(_, payload, _) = &chunks[0] else {
                unreachable!()
            };
            let ray = ChunkedPayload::try_from(payload).unwrap().ray_id();
            let mut variant = chunk(
                root.to_header(),
                0,
                raptorq::ObjectTransmissionInformation::with_defaults(
                    20_000, 1400,
                ),
                0,
                0,
            );
            let Message::Broadcast(_, ref mut payload, _) = variant else {
                unreachable!()
            };
            payload.gossip_frame[..RAY_ID_SIZE].copy_from_slice(&ray);
            payload.height = 7;
            dec.decode(variant, a).unwrap();
            let mut decoded_count = 0;
            for (index, msg) in chunks.into_iter().enumerate() {
                if let Some(Message::Broadcast(_, payload, _)) =
                    dec.decode(msg, if index % 2 == 0 { a } else { b }).unwrap()
                {
                    assert_eq!(payload.gossip_frame, original);
                    assert_eq!(payload.height, 7);
                    decoded_count += 1;
                }
            }
            assert_eq!(decoded_count, 1);
            assert!(dec.receiving.is_empty());
            assert!(dec.processed.entries.contains_key(&ray));
            assert!(dec.processed.entries.len() <= 3);
            assert_eq!(dec.processed.entries.len(), dec.processed.expiry.len());
            assert!(dec.heights.entries.len() <= 3);
            assert_eq!(dec.heights.entries.len(), dec.heights.expiry.len());
            assert_eq!(dec.cached_bytes(), 0);
            assert!(dec.source_usage.is_empty());
        }
    }

    #[test]
    fn expiration_runs_before_invalid_input_can_return_early() {
        let root = PeerNode::generate("192.0.2.1:9000", 0).unwrap();
        let source = root.value().address().ip();
        let info =
            raptorq::ObjectTransmissionInformation::with_defaults(20_000, 1300);
        let mut dec = RaptorQDecoder::configure(&RaptorQDecoderConf::default());
        dec.decode(chunk(root.to_header(), 0, info, 0, 0), source)
            .unwrap();
        dec.receiving.values_mut().next().unwrap().expire_on = Instant::now();
        dec.last_pruned -= dec.conf.cache_prune_every;
        assert!(
            dec.decode(
                Message::broadcast(
                    root.to_header(),
                    BroadcastPayload {
                        height: 0,
                        gossip_frame: vec![],
                    }
                ),
                source
            )
            .is_err()
        );
        assert!(dec.receiving.is_empty());
        assert_eq!(dec.cached_bytes(), 0);
        assert!(dec.source_usage.is_empty());
    }

    #[test]
    fn legacy_configuration_gets_safe_limits() {
        let conf: RaptorQDecoderConf =
            toml::from_str("cache_ttl = '1m'\ncache_prune_every = '5m'")
                .unwrap();
        let default = RaptorQDecoderConf::default();
        assert_eq!(conf.max_udp_len, default.max_udp_len);
        assert_eq!(conf.max_decoders, default.max_decoders);
        assert_eq!(conf.max_cache_bytes, default.max_cache_bytes);
        assert_eq!(
            conf.max_cache_bytes_per_source,
            default.max_cache_bytes_per_source
        );
        assert_eq!(conf.cache_prune_every, Duration::from_secs(300));
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
            dec.decode(n, root.value().address().ip())?;
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
                dec.decode(n, root.value().address().ip())?;
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
            dec.decode(n, root.value().address().ip())?;
        }
        assert_eq!(dec.cache_size(), 1);
        Ok(())
    }
}

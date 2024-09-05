// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::collections::HashMap;

use bucket::Bucket;
pub use bucket::InsertError;
pub use bucket::InsertOk;
pub use bucket::{NodeInsertError, NodeInsertOk};
use itertools::Itertools;
pub use key::{BinaryID, BinaryKey, BinaryNonce};
pub use node::Node;
use std::collections::hash_map::Entry;
use tracing::info;

mod bucket;
mod key;
mod node;
use crate::config::BucketConfig;
use crate::K_ALPHA;
use crate::K_BETA;

pub type BucketHeight = u8;

pub(crate) struct Tree<V> {
    root: Node<V>,
    buckets: HashMap<BucketHeight, Bucket<V>>,
    config: BucketConfig,
}

impl<V> Tree<V> {
    pub fn insert(
        &mut self,
        node: Node<V>,
    ) -> Result<InsertOk<V>, InsertError<V>> {
        if self.root().network_id != node.network_id {
            return Err(NodeInsertError::MismatchNetwork(node));
        }
        match self.root.calculate_distance(&node) {
            None => Err(NodeInsertError::Invalid(node)),
            Some(height) => self.get_or_create_bucket(height).insert(node),
        }
    }

    pub fn refresh(
        &mut self,
        node: Node<V>,
    ) -> Result<InsertOk<V>, InsertError<V>> {
        if self.root().network_id != node.network_id {
            return Err(NodeInsertError::MismatchNetwork(node));
        }
        match self.root.calculate_distance(&node) {
            None => Err(NodeInsertError::Invalid(node)),
            Some(height) => self.get_or_create_bucket(height).refresh(node),
        }
    }

    fn get_or_create_bucket(&mut self, height: BucketHeight) -> &mut Bucket<V> {
        return match self.buckets.entry(height) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(Bucket::new(self.config)),
        };
    }

    // iter the buckets (up to max_height, inclusive) and pick at most Beta
    // nodes for each bucket
    pub(crate) fn extract(
        &self,
        max_h: Option<BucketHeight>,
    ) -> impl Iterator<Item = (BucketHeight, impl Iterator<Item = &Node<V>>)>
    {
        let max_h = max_h.unwrap_or(BucketHeight::MAX);
        self.buckets
            .iter()
            .filter(move |(&height, _)| height <= max_h)
            .map(|(&height, bucket)| (height, bucket.pick::<K_BETA>()))
    }

    pub(crate) fn root(&self) -> &Node<V> {
        &self.root
    }

    pub(crate) fn closest_peers<const ITEM_COUNT: usize>(
        &self,
        other: &BinaryKey,
    ) -> impl Iterator<Item = &Node<V>> {
        self.buckets
            .iter()
            .flat_map(|(_, b)| b.peers())
            .filter(|p| p.id().as_binary() != other)
            .sorted_by(|a, b| {
                let distance_a = a.id().calculate_distance(other);
                let distance_b = b.id().calculate_distance(other);
                distance_a.cmp(&distance_b)
            })
            .take(ITEM_COUNT)
    }

    pub(crate) fn all_sorted(
        &self,
    ) -> impl Iterator<Item = (BucketHeight, impl Iterator<Item = &Node<V>>)>
    {
        self.buckets
            .iter()
            .sorted_by(|&(a, _), &(b, _)| a.cmp(b))
            .map(|(&height, bucket)| (height, bucket.peers()))
    }

    #[allow(dead_code)]
    pub(crate) fn idle_or_empty_heigth(
        &'static self,
    ) -> impl Iterator<Item = BucketHeight> {
        let max_buckets = (crate::K_ID_LEN_BYTES * 8) as BucketHeight;
        (0..max_buckets).filter(move |h| {
            self.buckets.get(h).map_or_else(|| true, |b| b.has_idle())
        })
    }

    // pick at most Alpha nodes for each idle bucket
    pub(crate) fn idle_buckets(
        &self,
    ) -> impl Iterator<Item = (BucketHeight, impl Iterator<Item = &Node<V>>)>
    {
        self.buckets
            .iter()
            .filter(|(_, bucket)| bucket.has_idle())
            .map(|(&height, bucket)| (height, bucket.pick::<K_ALPHA>()))
    }

    // Return the height of a Peer
    pub(crate) fn has_peer(&self, peer: &BinaryKey) -> Option<BucketHeight> {
        self.root.id().calculate_distance(peer).and_then(|height| {
            self.buckets
                .get(&height)
                .and_then(|bucket| bucket.has_node(peer).then_some(height))
        })
    }

    pub(crate) fn remove_peer(&mut self, peer: &BinaryKey) -> Option<Node<V>> {
        self.root.id().calculate_distance(peer).and_then(|height| {
            self.buckets
                .get_mut(&height)
                .and_then(|bucket| bucket.remove_id(peer))
        })
    }

    pub(crate) fn idle_nodes(&self) -> impl Iterator<Item = &Node<V>> {
        self.buckets.iter().flat_map(|(_, b)| b.idle_nodes())
    }

    pub(crate) fn remove_idle_nodes(&mut self) {
        self.buckets
            .iter_mut()
            .for_each(|(_, b)| b.remove_idle_nodes())
    }

    pub(crate) fn alive_nodes(&self) -> impl Iterator<Item = &Node<V>> {
        self.buckets
            .iter()
            .flat_map(|(_, bucket)| bucket.alive_nodes())
    }

    pub(crate) fn is_bucket_full(&self, height: BucketHeight) -> bool {
        self.buckets
            .get(&height)
            .map_or(false, |bucket| bucket.is_full())
    }

    pub(crate) fn new(root: Node<V>, config: BucketConfig) -> Tree<V> {
        info!(
            "Building table [K={}] with root: {:?}",
            crate::K_K,
            root.id()
        );
        Tree {
            root,
            config,
            buckets: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::peer::PeerNode;
    use crate::tests::Result;
    #[test]
    fn test_buckets() -> Result<()> {
        let root = PeerNode::generate("192.168.0.1:666", 0)?;
        let mut config = BucketConfig::default();
        config.node_evict_after = Duration::from_millis(5000);
        config.node_ttl = Duration::from_secs(60);

        let mut route_table = Tree::new(root, config);
        for i in 2..255 {
            let node = PeerNode::generate(format!("192.168.0.{}:666", i), 0)?;
            let res = route_table.insert(node);
            match res {
                Ok(_) | Err(NodeInsertError::Full(_)) => {}
                _ => panic!("Node must be valid"),
            }
        }
        let res = route_table
            .insert(PeerNode::generate("192.168.0.1:666", 0)?)
            .expect_err("this should be an error");

        assert!(if let NodeInsertError::Invalid(_) = res {
            true
        } else {
            false
        });
        Ok(())
    }
}

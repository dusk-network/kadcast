// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::collections::HashMap;

use bucket::Bucket;
pub use bucket::{NodeInsertError, NodeInsertOk};
use itertools::Itertools;
pub use key::{BinaryID, BinaryKey, BinaryNonce};
pub use node::Node;

pub use bucket::InsertError;
pub use bucket::InsertOk;
use tracing::info;

mod bucket;
mod key;
mod node;
use crate::config::BucketConfig;
use crate::K_ALPHA;
use crate::K_BETA;
use crate::K_ID_LEN_BYTES;

pub type BucketHeight = usize;

pub(crate) struct Tree<V> {
    root: Node<V>,
    buckets: HashMap<BucketHeight, Bucket<V>>,
    pub(crate) config: BucketConfig,
}

impl<V> Tree<V> {
    pub fn insert(
        &mut self,
        node: Node<V>,
    ) -> Result<InsertOk<V>, InsertError<V>> {
        match self.root.calculate_distance(&node) {
            None => Err(NodeInsertError::Invalid(node)),
            Some(height) => self.get_or_create_bucket(height).insert(node),
        }
    }

    fn get_or_create_bucket(&mut self, height: BucketHeight) -> &mut Bucket<V> {
        return match self.buckets.entry(height) {
            std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(Bucket::new(self.config))
            }
        };
    }

    //iter the buckets (up to max_height, inclusive) and pick at most Beta
    // nodes for each bucket
    pub(crate) fn extract(
        &self,
        max_h: Option<usize>,
    ) -> impl Iterator<Item = (BucketHeight, impl Iterator<Item = &Node<V>>)>
    {
        self.buckets
            .iter()
            .filter(move |(&height, _)| height <= max_h.unwrap_or(usize::MAX))
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
                Ord::cmp(
                    &a.id().calculate_distance(other),
                    &b.id().calculate_distance(other),
                )
            })
            .take(ITEM_COUNT)
    }

    pub(crate) fn all_sorted(
        &self,
    ) -> impl Iterator<Item = (BucketHeight, impl Iterator<Item = &Node<V>>)>
    {
        self.buckets
            .iter()
            .sorted_by(|a, b| Ord::cmp(&a.0, &b.0))
            .map(|(&height, bucket)| (height, bucket.peers()))
    }

    #[allow(dead_code)]
    pub(crate) fn idle_or_empty_heigth(
        &'static self,
    ) -> impl Iterator<Item = BucketHeight> {
        (0..K_ID_LEN_BYTES * 8).filter(move |h| {
            self.buckets.get(h).map_or_else(|| true, |b| b.is_idle())
        })
    }

    //pick at most Alpha nodes for each idle bucket
    pub(crate) fn idle_buckets(
        &self,
    ) -> impl Iterator<Item = (BucketHeight, impl Iterator<Item = &Node<V>>)>
    {
        self.buckets
            .iter()
            .filter(move |(_, bucket)| bucket.is_idle())
            .map(|(&height, bucket)| (height, bucket.pick::<K_ALPHA>()))
    }

    pub(crate) fn has_peer(&self, peer: &BinaryKey) -> Option<usize> {
        match self.root.id().calculate_distance(peer) {
            None => None,
            Some(height) => self.buckets.get(&height).and_then(|bucket| {
                if bucket.has_node(peer) {
                    Some(height)
                } else {
                    None
                }
            }),
        }
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

    pub(crate) fn is_bucket_full(&self, height: usize) -> bool {
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

// pub struct TreeBuilder<V> {
//     node_ttl: Duration,
//     node_evict_after: Duration,
//     root: Node<V>,
//     bucket_ttl: Duration,
// }

// impl<V> TreeBuilder<V> {
//     pub(crate) fn new(root: Node<V>) -> TreeBuilder<V> {
//         TreeBuilder {
//             root,
//             node_evict_after: Duration::from_millis(
//                 BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS,
//             ),
//             node_ttl: Duration::from_millis(BUCKET_DEFAULT_NODE_TTL_MILLIS),
//             bucket_ttl: Duration::from_secs(BUCKET_DEFAULT_TTL_SECS),
//         }
//     }

//     pub fn with_node_ttl(mut self, node_ttl: Duration) -> TreeBuilder<V> {
//         self.node_ttl = node_ttl;
//         self
//     }

//     pub fn with_bucket_ttl(mut self, bucket_ttl: Duration) -> TreeBuilder<V>
// {         self.bucket_ttl = bucket_ttl;
//         self
//     }

//     pub fn with_node_evict_after(
//         mut self,
//         node_evict_after: Duration,
//     ) -> TreeBuilder<V> {
//         self.node_evict_after = node_evict_after;
//         self
//     }

//     pub(crate) fn build(self) -> Tree<V> {
//         info!(
//             "Built table [K={}] with root: {:?}",
//             crate::K_K,
//             self.root.id()
//         );
//         Tree {
//             root: self.root,
//             buckets: HashMap::new(),
//             config: BucketConfig {
//                 bucket_ttl: self.bucket_ttl,
//                 node_evict_after: self.node_evict_after,
//                 node_ttl: self.node_ttl,
//             },
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        config::BucketConfig,
        kbucket::{NodeInsertError, Tree},
        peer::PeerNode,
    };

    #[test]
    fn it_works() {
        let root = PeerNode::from_address("192.168.0.1:666");
        let mut config = BucketConfig::default();
        config.node_evict_after = Duration::from_millis(5000);
        config.node_ttl = Duration::from_secs(60);

        let mut route_table = Tree::new(root, config);
        for i in 2..255 {
            let res = route_table.insert(PeerNode::from_address(
                &format!("192.168.0.{}:666", i)[..],
            ));
            match res {
                Ok(_) => {}
                Err(error) => match error {
                    NodeInsertError::Invalid(_) => {
                        assert!(false)
                    }
                    _ => {}
                },
            }
        }
        let res = route_table
            .insert(PeerNode::from_address("192.168.0.1:666"))
            .expect_err("this should be an error");
        assert!(if let NodeInsertError::Invalid(_) = res {
            true
        } else {
            false
        });
    }
}

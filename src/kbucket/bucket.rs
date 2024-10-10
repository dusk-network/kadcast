// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use arrayvec::ArrayVec;
use rand::seq::SliceRandom;
use rand::thread_rng;
use semver::Version;

use super::node::{Node, NodeEvictionStatus};
use super::BinaryKey;
use crate::config::BucketConfig;
use crate::K_K;

/// Represents a bucket for storing nodes in a Kademlia routing table.
pub(super) struct Bucket<V> {
    nodes: arrayvec::ArrayVec<Node<V>, K_K>,
    pending_node: Option<Node<V>>,
    bucket_config: BucketConfig,
}

/// Enum representing the result of inserting a node into a bucket.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeInsertOk<'a, TNode> {
    /// The node was successfully inserted into the bucket.
    Inserted { inserted: &'a TNode },
    /// The node was updated within the bucket, and a potential eviction is
    /// pending.
    Updated {
        updated: &'a TNode,
        pending_eviction: Option<&'a TNode>,
    },
    /// The insertion of the node is pending, and a potential eviction is
    /// pending as well.
    Pending {
        pending_insert: &'a TNode,
        pending_eviction: Option<&'a TNode>,
    },

    /// No action has been performed
    NoAction,
}

/// Enum representing possible errors when inserting a node into a bucket.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeInsertError<TNode> {
    /// The node is considered invalid and cannot be inserted.
    Invalid(TNode),
    /// The bucket is already full, and the node cannot be inserted.
    Full(TNode),
    /// There is a mismatch with the network while inserting the node.
    MismatchNetwork(TNode),
    /// There is a mismatch with the version while inserting the node.
    MismatchVersion(TNode, Version),
}

impl<'a, TNode> NodeInsertOk<'a, TNode> {
    /// Returns an optional reference to the node pending eviction.
    pub fn pending_eviction(&self) -> Option<&TNode> {
        match self {
            NodeInsertOk::Inserted { inserted: _ } => None,
            NodeInsertOk::Pending {
                pending_insert: _,
                pending_eviction,
            } => *pending_eviction,
            NodeInsertOk::Updated {
                updated: _,
                pending_eviction,
            } => *pending_eviction,
            Self::NoAction => None,
        }
    }
}

/// Type alias for a successful node insertion result in a bucket.
pub type InsertOk<'a, V> = NodeInsertOk<'a, Node<V>>;

/// Type alias for an error during node insertion into a bucket.
pub type InsertError<V> = NodeInsertError<Node<V>>;

impl<V> Bucket<V> {
    /// Creates a new `Bucket` with the given configuration.
    pub(super) fn new(bucket_config: BucketConfig) -> Self {
        Bucket {
            nodes: ArrayVec::<Node<V>, K_K>::new(),
            pending_node: None,
            bucket_config,
        }
    }

    /// Refreshes the last usage time of a node based on the given key and
    /// returns a reference to it.
    fn refresh_node(&mut self, key: &BinaryKey) -> Option<&Node<V>> {
        let old_index =
            self.nodes.iter().position(|s| s.id().as_binary() == key)?;
        self.nodes[old_index..].rotate_left(1);
        self.nodes.last_mut()?.refresh();
        self.nodes.last()
    }

    /// Attempts to insert a previously marked pending node into the bucket. No
    /// action is taken if the bucket is full.
    ///
    /// If the pending node is no longer alive, it is removed.
    fn insert_pending(&mut self) {
        if self.nodes.is_full() {
            return;
        };
        if let Some(pending) = self.pending_node.take() {
            if pending.is_alive(self.bucket_config.node_ttl) {
                // FIXME: Consider using `try_push` instead of `push`.
                // FIXME2: This may break the LRU policy, as other records may
                // have been updated in the meantime. However, it is mitigated
                // by the `is_alive` check.
                self.nodes.push(pending);
            }
        }
    }

    /// If the bucket is full, flag the least recently used node for eviction.
    /// If it's already flagged, check if the timeout has expired and then
    /// replace it with the pending node.
    ///
    /// The method returns the candidate for eviction (if any).
    fn try_perform_eviction(&mut self) -> Option<&Node<V>> {
        if !self.nodes.is_full() {
            return None;
        }
        match self.nodes.first()?.eviction_status {
            NodeEvictionStatus::Requested(instant) => {
                if instant.elapsed() < self.bucket_config.node_evict_after {
                    self.nodes.first()
                } else {
                    self.nodes.pop_at(0);
                    self.insert_pending();
                    None
                }
            }
            NodeEvictionStatus::None => {
                if self.nodes.first()?.is_alive(self.bucket_config.node_ttl) {
                    None
                } else {
                    self.nodes.first_mut()?.flag_for_check();
                    self.nodes.first()
                }
            }
        }
    }

    /// Tries to insert a node into the bucket and returns the result.
    pub fn insert(
        &mut self,
        node: Node<V>,
    ) -> Result<InsertOk<V>, InsertError<V>> {
        if !node.id().verify_nonce() {
            return Err(NodeInsertError::Invalid(node));
        }
        if self.refresh_node(node.id().as_binary()).is_some() {
            self.try_perform_eviction();
            return Ok(NodeInsertOk::Updated {
                pending_eviction: self.pending_eviction_node(),
                updated: self.nodes.last().expect(
                    "last node to exist because it's been just updated",
                ),
            });
        }
        self.try_perform_eviction();
        match self.nodes.try_push(node) {
            Ok(_) => Ok(NodeInsertOk::Inserted {
                inserted: self.nodes.last().expect(
                    "last node to exist because it's been just inserted",
                ),
            }),
            Err(err) => {
                if self
                    .nodes
                    .first()
                    .expect("Bucket full but no node as .first()")
                    .is_alive(self.bucket_config.node_ttl)
                {
                    Err(NodeInsertError::Full(err.element()))
                } else {
                    self.pending_node = Some(err.element());
                    Ok(NodeInsertOk::Pending {
                        pending_insert: self
                            .pending_node
                            .as_ref()
                            .expect("Unable to get the pending node back"),
                        pending_eviction: self.pending_eviction_node(),
                    })
                }
            }
        }
    }

    /// Tries to refresh a node.
    pub fn refresh(
        &mut self,
        node: Node<V>,
    ) -> Result<InsertOk<V>, InsertError<V>> {
        if !node.id().verify_nonce() {
            return Err(NodeInsertError::Invalid(node));
        }

        if self.refresh_node(node.id().as_binary()).is_none() {
            return Ok(NodeInsertOk::NoAction);
        }

        self.try_perform_eviction();
        Ok(NodeInsertOk::Updated {
            pending_eviction: self.pending_eviction_node(),
            updated: self
                .nodes
                .last()
                .expect("last node to exist because it's been just updated"),
        })
    }

    /// Picks at most `ITEM_COUNT` random nodes from this bucket.
    pub fn pick<const ITEM_COUNT: usize>(
        &self,
    ) -> impl Iterator<Item = &Node<V>> {
        let mut idxs: Vec<_> = (0..self.nodes.len()).collect();
        idxs.shuffle(&mut thread_rng());
        idxs.into_iter()
            .take(ITEM_COUNT)
            .filter_map(move |idx| self.nodes.get(idx))
    }

    /// Returns the least recently used node to query if flagged for eviction.
    fn pending_eviction_node(&self) -> Option<&Node<V>> {
        self.nodes
            .first()
            .filter(|n| n.eviction_status != NodeEvictionStatus::None)
    }

    /// Returns an iterator over the peers in the bucket.
    pub(super) fn peers(&self) -> impl Iterator<Item = &Node<V>> {
        self.nodes.iter()
    }

    /// Checks if the bucket has at least one idle node.
    pub(crate) fn has_idle(&self) -> bool {
        self.nodes.first().map_or(false, |n| {
            n.seen_at.elapsed() > self.bucket_config.bucket_ttl
        })
    }

    /// Removes idle nodes from the bucket.
    pub(crate) fn remove_idle_nodes(&mut self) {
        let ttl = self.bucket_config.node_ttl;
        self.nodes.retain(|n| n.is_alive(ttl));
        self.insert_pending();
    }

    /// Returns an iterator over the alive nodes in the bucket.
    pub(crate) fn alive_nodes(&self) -> impl Iterator<Item = &Node<V>> {
        let ttl = self.bucket_config.node_ttl;
        self.nodes.iter().filter(move |&n| n.is_alive(ttl))
    }

    /// Get idle nodes from the bucket.
    pub(crate) fn idle_nodes(&self) -> impl Iterator<Item = &Node<V>> {
        let ttl = self.bucket_config.node_ttl;
        self.nodes.iter().filter(move |n| !n.is_alive(ttl))
    }

    /// Checks if the bucket contains a node with the given peer key.
    pub(crate) fn has_node(&self, peer: &BinaryKey) -> bool {
        self.nodes.iter().any(|n| n.id().as_binary() == peer)
    }

    /// Checks if the bucket is full.
    pub(crate) fn is_full(&self) -> bool {
        self.nodes.is_full()
    }

    /// Removes a node from the bucket by its ID.
    ///
    /// If there is an alive pending node, it's inserted.
    ///
    /// Returns the removed node.
    pub(crate) fn remove_id(&mut self, id: &[u8]) -> Option<Node<V>> {
        let node_idx =
            self.nodes.iter().position(|s| s.id().as_binary() == id)?;

        self.nodes.pop_at(node_idx).map(|removed| {
            if let Some(pending) = self.pending_node.take() {
                if pending.is_alive(self.bucket_config.node_ttl) {
                    self.nodes.push(pending);
                }
            }
            removed
        })
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::kbucket::Tree;
    use crate::peer::PeerNode;
    use crate::tests::Result;
    use crate::K_BETA;

    impl<V> Bucket<V> {
        pub fn last_id(&self) -> Option<&BinaryKey> {
            self.nodes.last().map(|n| n.id().as_binary())
        }

        pub fn least_used_id(&self) -> Option<&BinaryKey> {
            self.nodes.first().map(|n| n.id().as_binary())
        }
    }

    impl<V> crate::kbucket::Tree<V> {
        fn bucket_for_test(&mut self) -> &mut Bucket<V> {
            self.get_or_create_bucket(1)
        }
    }

    #[test]
    fn test_lru_base_5secs() -> Result<()> {
        let root = PeerNode::generate("127.0.0.1:666", 0)?;
        let mut config = BucketConfig::default();
        config.node_evict_after = Duration::from_millis(1000);
        config.node_ttl = Duration::from_secs(5);

        let mut route_table = Tree::new(root, config);

        let bucket = route_table.bucket_for_test();
        let node1 = PeerNode::generate("192.168.1.1:8080", 0)?;
        let id_node1 = node1.id().as_binary().clone();
        let node1_copy = PeerNode::generate("192.168.1.1:8080", 0)?;
        match bucket.insert(node1).expect("This should return an ok()") {
            NodeInsertOk::Inserted { .. } => {}
            _ => assert!(false),
        }
        let a = bucket.pick::<K_BETA>();
        assert_eq!(a.count(), 1);

        match bucket
            .insert(node1_copy)
            .expect("This should return an ok()")
        {
            NodeInsertOk::Updated { .. } => {}
            _ => assert!(false),
        }
        assert_eq!(Some(&id_node1), bucket.last_id());
        let node2 = PeerNode::generate("192.168.1.2:8080", 0)?;
        let id_node2 = node2.id().as_binary().clone();

        match bucket.insert(node2).expect("This should return an ok()") {
            NodeInsertOk::Inserted { inserted: _ } => {}
            _ => assert!(false),
        }
        let a = bucket.pick::<K_BETA>();
        assert_eq!(a.count(), 2);
        assert_eq!(Some(&id_node2), bucket.last_id());
        assert_eq!(Some(&id_node1), bucket.least_used_id());

        match bucket
            .insert(PeerNode::generate("192.168.1.1:8080", 0)?)
            .expect("This should return an ok()")
        {
            NodeInsertOk::Updated { .. } => {}
            _ => assert!(false),
        }
        let a = bucket.pick::<K_BETA>();
        assert_eq!(a.count(), 2);
        assert_eq!(Some(&id_node1), bucket.last_id());
        assert_eq!(Some(&id_node2), bucket.least_used_id());
        let a = bucket.remove_id(&id_node2);
        assert_eq!(&id_node2, a.unwrap().id().as_binary());
        assert_eq!(Some(&id_node1), bucket.last_id());
        assert_eq!(Some(&id_node1), bucket.least_used_id());
        for i in 2..21 {
            match bucket
                .insert(PeerNode::generate(
                    &format!("192.168.1.{}:8080", i)[..],
                    0,
                )?)
                .expect("This should return an ok()")
            {
                NodeInsertOk::Inserted { .. } => {
                    assert!(bucket.pick::<K_BETA>().count() <= K_BETA);
                }
                _ => assert!(false),
            }
        }
        assert_eq!(bucket.pick::<K_BETA>().count(), K_BETA);
        let pending = PeerNode::generate("192.168.1.21:8080", 0)?;
        let pending_id = pending.id().as_binary().clone();
        match bucket.insert(pending).expect_err("this should be error") {
            NodeInsertError::Full(pending) => {
                assert_eq!(pending.id().as_binary(), &pending_id);
                thread::sleep(Duration::from_secs(5));
                match bucket.insert(pending).expect("this should be ok") {
                    NodeInsertOk::Pending {
                        pending_insert,
                        pending_eviction: _,
                    } => {
                        assert_eq!(
                            pending_insert.id().as_binary(),
                            &pending_id
                        );
                        thread::sleep(Duration::from_secs(1));
                        let pending =
                            PeerNode::generate("192.168.1.21:8080", 0)?;
                        match bucket.insert(pending).expect("this should be ok")
                        {
                            NodeInsertOk::Inserted { inserted: _ } => {}
                            v => {
                                println!("{:?}", v);
                                assert!(false)
                            }
                        }
                    }
                    b => {
                        println!("{:?}", b);
                        assert!(false)
                    }
                }
            }
            _ => assert!(false),
        }
        Ok(())
    }
}

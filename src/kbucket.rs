use std::iter;
use std::time::Duration;
mod bucket;
mod key;
mod node;
use bucket::{Bucket, BucketConfig};
pub use bucket::{NodeInsertError, NodeInsertOk};
pub use key::{BinaryID, BinaryKey, BinaryNonce};
pub use node::Node;

use crate::K_BUCKETS_AMOUNT;

pub use bucket::InsertError;
pub use bucket::InsertOk;

const BUCKET_DEFAULT_NODE_TTL_MILLIS: u64 = 30000;
const BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS: u64 = 5000;

pub type BucketHeight = usize;

pub struct Tree<V> {
    root: Node<V>,
    buckets: arrayvec::ArrayVec<Bucket<V>, K_BUCKETS_AMOUNT>,
}

impl<V> Tree<V> {
    pub fn insert(&mut self, node: Node<V>) -> Result<InsertOk<V>, InsertError<V>> {
        let bucket_idx = self.root.calculate_distance(&node);
        match bucket_idx {
            None => Err(NodeInsertError::Invalid(node)),
            Some(idx) => self.buckets[idx].insert(node),
        }
    }

    //iter the buckets (up to max_height) and pick at most Beta nodes for each bucket
    pub fn extract(
        &self,
        max_h: usize,
    ) -> impl Iterator<Item = (BucketHeight, impl Iterator<Item = &Node<V>>)> {
        self.buckets
            .iter()
            .take(max_h)
            .enumerate()
            .map(|(idx, bucket)| (idx, bucket.pick()))
    }

    pub fn builder(root: Node<V>) -> TreeBuilder<V> {
        TreeBuilder::new(root)
    }
}

pub struct TreeBuilder<V> {
    node_ttl: Duration,
    node_evict_after: Duration,
    root: Node<V>,
}

impl<V> TreeBuilder<V> {
    fn new(root: Node<V>) -> TreeBuilder<V> {
        TreeBuilder {
            root,
            node_evict_after: Duration::from_millis(BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS),
            node_ttl: Duration::from_millis(BUCKET_DEFAULT_NODE_TTL_MILLIS),
        }
    }

    pub fn node_ttl(mut self, node_ttl: Duration) -> TreeBuilder<V> {
        self.node_ttl = node_ttl;
        self
    }

    pub fn node_evict_after(mut self, node_evict_after: Duration) -> TreeBuilder<V> {
        self.node_evict_after = node_evict_after;
        self
    }

    pub fn build(self) -> Tree<V> {
        let config = BucketConfig::new(self.node_ttl, self.node_evict_after);
        Tree {
            root: self.root,
            buckets: iter::repeat_with(|| Bucket::new(config))
                .take(K_BUCKETS_AMOUNT)
                .collect(),
        }
    }
}

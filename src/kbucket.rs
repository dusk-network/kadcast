use std::collections::HashMap;
use std::time::Duration;
mod bucket;
mod key;
mod node;
use bucket::{Bucket, BucketConfig};
pub use bucket::{NodeInsertError, NodeInsertOk};
pub use key::{BinaryID, BinaryKey, BinaryNonce};
pub use node::Node;

pub use bucket::InsertError;
pub use bucket::InsertOk;

const BUCKET_DEFAULT_NODE_TTL_MILLIS: u64 = 30000;
const BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS: u64 = 5000;

pub type BucketHeight = usize;

pub struct Tree<V> {
    root: Node<V>,
    buckets: HashMap<BucketHeight, Bucket<V>>,
    config: BucketConfig,
}

impl<V> Tree<V> {
    pub fn insert(&mut self, node: Node<V>) -> Result<InsertOk<V>, InsertError<V>> {
        match self.root.calculate_distance(&node) {
            None => Err(NodeInsertError::Invalid(node)),
            Some(height) => self.safe_get_bucket(height).insert(node),
        }
    }

    fn safe_get_bucket(&mut self, height: BucketHeight) -> &mut Bucket<V> {
        return match self.buckets.entry(height) {
            std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
            std::collections::hash_map::Entry::Vacant(v) => v.insert(Bucket::new(self.config)),
        };
    }

    //iter the buckets (up to max_height, inclusive) and pick at most Beta nodes for each bucket
    pub fn extract(
        &self,
        max_h: usize,
    ) -> impl Iterator<Item = (BucketHeight, impl Iterator<Item = &Node<V>>)> {
        self.buckets
            .iter()
            .filter(move |(&height, _)| height <= max_h)
            .map(|(&height, bucket)| (height, bucket.pick()))
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
            buckets: HashMap::new(),
            config,
        }
    }
}

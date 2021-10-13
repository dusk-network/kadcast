use std::collections::HashMap;
use std::time::Duration;
mod bucket;
mod key;
mod node;
use bucket::{Bucket, BucketConfig};
pub use bucket::{NodeInsertError, NodeInsertOk};
use itertools::Itertools;
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
            Some(height) => self.get_or_create_bucket(height).insert(node),
        }
    }

    fn get_or_create_bucket(&mut self, height: BucketHeight) -> &mut Bucket<V> {
        return match self.buckets.entry(height) {
            std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
            std::collections::hash_map::Entry::Vacant(v) => v.insert(Bucket::new(self.config)),
        };
    }

    //iter the buckets (up to max_height, inclusive) and pick at most Beta nodes for each bucket
    pub(crate) fn extract(
        &self,
        max_h: Option<usize>,
    ) -> impl Iterator<Item = (BucketHeight, impl Iterator<Item = &Node<V>>)> {
        self.buckets
            .iter()
            .filter(move |(&height, _)| height <= max_h.unwrap_or(usize::MAX))
            .map(|(&height, bucket)| (height, bucket.pick()))
    }

    pub(crate) fn root(&self) -> &Node<V> {
        &self.root
    }

    pub(crate) fn closest_peers(&self, other: &BinaryID) -> impl Iterator<Item = &Node<V>> {
        self.buckets
            .iter()
            .flat_map(|(_, b)| b.peers())
            .filter(|p| p.id() != other)
            .sorted_by(|a, b| {
                Ord::cmp(
                    &a.id().calculate_distance(other),
                    &b.id().calculate_distance(other),
                )
            })
            .take(crate::K_K)
    }
}

pub struct TreeBuilder<V> {
    node_ttl: Duration,
    node_evict_after: Duration,
    root: Node<V>,
}

impl<V> TreeBuilder<V> {
    pub(crate) fn new(root: Node<V>) -> TreeBuilder<V> {
        TreeBuilder {
            root,
            node_evict_after: Duration::from_millis(BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS),
            node_ttl: Duration::from_millis(BUCKET_DEFAULT_NODE_TTL_MILLIS),
        }
    }

    pub fn set_node_ttl(mut self, node_ttl: Duration) -> TreeBuilder<V> {
        self.node_ttl = node_ttl;
        self
    }

    pub fn set_node_evict_after(mut self, node_evict_after: Duration) -> TreeBuilder<V> {
        self.node_evict_after = node_evict_after;
        self
    }

    pub(crate) fn build(self) -> Tree<V> {
        let config = BucketConfig::new(self.node_ttl, self.node_evict_after);
        println!("Built table with root: {:?}", self.root.id());
        Tree {
            root: self.root,
            buckets: HashMap::new(),
            config,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        kbucket::{NodeInsertError, TreeBuilder},
        peer::PeerNode,
    };

    #[test]
    fn it_works() {
        let root = PeerNode::from_address("192.168.0.1:666");
        let mut route_table = TreeBuilder::new(root)
            .set_node_evict_after(Duration::from_millis(5000))
            .set_node_ttl(Duration::from_secs(60))
            .build();
        for i in 2..255 {
            let res =
                route_table.insert(PeerNode::from_address(&format!("192.168.0.{}:666", i)[..]));
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

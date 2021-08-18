use std::iter;
mod bucket;
mod key;
mod node;
pub use bucket::Bucket;
pub use bucket::NodeInsertOk;
pub use bucket::NodeInsertError;
pub use key::BinaryID;
pub use key::BinaryKey;
pub use key::BinaryNonce;
pub use bucket::BucketConfig;
pub use node::Node;

use crate::K_BUCKETS_AMOUNT;

pub use bucket::InsertOk;
pub use bucket::InsertError;



pub struct Tree<ID: BinaryID, V> {
    root: Node<ID, V>,
    buckets: arrayvec::ArrayVec<Bucket<ID, V>, K_BUCKETS_AMOUNT>,
}

impl<ID: BinaryID, V> Tree<ID, V> {
    pub fn new(root: Node<ID, V>, config: BucketConfig) -> Self {
        Tree {
            root,
            buckets: iter::repeat_with(|| Bucket::new(config))
                .take(K_BUCKETS_AMOUNT)
                .collect(),
        }
    }

    pub fn insert(&mut self, node: Node<ID, V>) -> Result<InsertOk<ID, V>, InsertError<ID, V>> {
        let bucket_idx = self.root.calculate_distance(&node);
        match bucket_idx {
            None => Err(NodeInsertError::Invalid(node)),
            Some(idx) => self.buckets[idx].insert(node),
        }
    }
}

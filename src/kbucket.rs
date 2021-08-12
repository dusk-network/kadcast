use std::iter;
mod bucket;
mod key;
mod node;
pub use bucket::Bucket;
pub use bucket::InsertResult;
pub use key::BinaryID;
pub use key::BinaryKey;
pub use key::BinaryNonce;
pub use node::Node;

use crate::K_BUCKETS_AMOUNT;
use crate::K_ID_LEN_BYTES;
use crate::K_NONCE_LEN;

pub struct Tree<ID: BinaryID, V> {
    root: Node<ID, V>,
    buckets: arrayvec::ArrayVec<Bucket<ID, V>, K_BUCKETS_AMOUNT>,
}

impl<ID: BinaryID, V> Tree<ID, V> {
    pub fn for_root(root: Node<ID, V>) -> Tree<ID, V> {
        Tree {
            root,
            buckets: iter::repeat_with(Bucket::new)
                .take(K_BUCKETS_AMOUNT)
                .collect(),
        }
    }

    pub fn insert(&mut self, node: Node<ID, V>) -> InsertResult<Node<ID, V>> {
        let bucket_idx = self.root.calculate_distance(&node);
        match bucket_idx {
            None => InsertResult::Invalid(node),
            Some(idx) => self.buckets[idx].insert(node),
        }
    }
}

use std::iter;

use crate::kbucket::{BinaryID, Bucket, InsertResult, Node};

pub struct Tree<ID: BinaryID, V> {
    root: Node<ID, V>,
    buckets: arrayvec::ArrayVec<Bucket<ID, V>, 128>,
}

impl<ID: BinaryID, V> Tree<ID, V> {
    pub fn for_root(root: Node<ID, V>) -> Tree<ID, V> {
        Tree {
            root,
            buckets: iter::repeat_with(Bucket::new).take(128).collect(),
        }
    }

    pub fn insert(&mut self, node: Node<ID, V>) -> InsertResult {
        let bucket_idx = self.root.calculate_distance(&node);
        bucket_idx.map_or(InsertResult::Invalid, |dist| {
            self.buckets[dist].insert(node)
        })
    }
}

use crate::{K_BETA, K_K};

use super::key::BinaryID;
use super::node::Node;
use arrayvec::ArrayVec;

pub struct Bucket<ID: BinaryID, V> {
    nodes: arrayvec::ArrayVec<Node<ID, V>, K_K>,
}

#[derive(Debug)]
pub enum InsertResult<TNode> {
    Inserted,
    Updated,
    Pending(TNode),
    Rejected(TNode),
    Invalid(TNode),
}

impl<ID: BinaryID, V> Bucket<ID, V> {
    pub fn new() -> Bucket<ID, V> {
        Bucket {
            nodes: ArrayVec::<Node<ID, V>, K_K>::new(),
        }
    }

    pub fn insert(&mut self, node: Node<ID, V>) -> InsertResult<Node<ID, V>> {
        if !node.is_id_valid() {
            return InsertResult::Invalid(node);
        }
        // self.nodes
        //     .iter()
        //     .position(|s| s.id().as_binary() == node.id().as_binary())
        //     .map(|old_index| self.nodes.pop_at(old_index));

        match self.nodes.try_push(node) {
            Ok(_) => InsertResult::Inserted,
            Err(err) => InsertResult::Pending(err.element()),
        }
    }

    pub fn pick(&self) -> [Option<Node<ID, V>>; K_BETA] {
        [None, None, None]
    }

    pub fn remove_last_unreach(&mut self) -> Option<Node<ID, V>> {
        //TODO
        None
    }
}


use crate::{K_BETA, K_K};

use super::node::Node;
use arrayvec::ArrayVec;
use super::key::BinaryID;

pub struct Bucket<ID:BinaryID, V> {
    nodes: arrayvec::ArrayVec<Node<ID, V>, K_K>,
}

#[derive(Debug)]
pub enum InsertResult {
    Inserted,
    Pending,
    Rejected,
    Invalid,
}

impl<ID:BinaryID, V> Bucket<ID, V> {

    pub fn new() -> Bucket<ID,V>{
        Bucket{
            nodes:ArrayVec::<Node<ID, V>, K_K>::new(),
        }
    }

    pub fn insert(&mut self, node: Node<ID, V>) -> InsertResult {
        if !node.is_id_valid() {
            return InsertResult::Invalid;
        }
        match self.nodes.try_push(node) {
            Ok(_) => InsertResult::Inserted,
            Err(e) => InsertResult::Rejected
        }
    }

    pub fn pick(&self) -> [Option<Node<ID, V>>; K_BETA] {
        return [None, None, None];
    }

    pub fn remove_last_unreach(&mut self) -> Option<Node<ID,V>>{
        //TODO
        None
    }
}
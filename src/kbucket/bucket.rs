use crate::{K_BETA, K_K};

use super::key::BinaryID;
use super::node::Node;
use arrayvec::ArrayVec;

pub struct Bucket<ID: BinaryID, V> {
    nodes: arrayvec::ArrayVec<Node<ID, V>, K_K>,
}

#[derive(Debug,PartialEq, Eq, PartialOrd, Ord)]
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
        let updated = self
            .nodes
            .iter()
            .position(|s| s.id().as_binary() == node.id().as_binary())
            .map(|old_index| {
                //Try to insert again an existing node, should move it at the end of our bucket according to LRU policy
                self.nodes[old_index..].rotate_left(1);
                InsertResult::Updated
            });

        updated.unwrap_or_else(|| match self.nodes.try_push(node) {
            Ok(_) => InsertResult::Inserted,
            Err(err) => InsertResult::Pending(err.element()),
        })
    }

    pub fn pick(&self) -> [Option<Node<ID, V>>; K_BETA] {
        [None, None, None]
    }

    pub fn last(&self) -> Option<&Node<ID,V>>{
        self.nodes.last()
    }

    pub fn remove_last_unreach(&mut self) -> Option<Node<ID, V>> {
        //TODO
        None
    }
}


#[cfg(test)]
mod tests {
    use crate::{kbucket::{Bucket, InsertResult}, peer};


    #[test]
    fn test_difficulty() {
        let mut bucket = Bucket::new();
        let node1 = peer::from_address("192.168.1.1:8080".to_string());
        let node2 = peer::from_address("192.168.1.1:8080".to_string());
        assert_eq!(InsertResult::Inserted, bucket.insert(node1));
        assert_eq!(InsertResult::Updated, bucket.insert(node2));
    }
}


use crate::{K_BETA, K_K};

use super::key::BinaryID;
use super::node::Node;
use arrayvec::ArrayVec;

pub struct Bucket<ID: BinaryID, V> {
    nodes: arrayvec::ArrayVec<Node<ID, V>, K_K>,
    pending_node: Option<Node<ID, V>>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InsertResult<'a, TNode> {
    Inserted,
    Updated,
    Pending(&'a TNode),
    Invalid(TNode),
}

impl<ID: BinaryID, V> Bucket<ID, V> {
    pub fn new() -> Bucket<ID, V> {
        Bucket {
            nodes: ArrayVec::<Node<ID, V>, K_K>::new(),
            pending_node: None,
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

        updated.unwrap_or_else(move || match self.nodes.try_push(node) {
            Ok(_) => InsertResult::Inserted,
            Err(err) => {
                self.pending_node = Some(err.element());
                let b = self.pending_node.as_ref().unwrap();
                InsertResult::Pending(b)
            }
        })
    }

    pub fn pick(&self) -> [Option<Node<ID, V>>; K_BETA] {
        [None, None, None]
    }

    pub fn least_used(&self) -> Option<&Node<ID, V>> {
        self.nodes.first()
    }

    pub fn remove_id(&mut self, id: &[u8]) -> Option<Node<ID, V>> {
        self.nodes
            .iter()
            .position(|s| s.id().as_binary() == id)
            .and_then(|old_index| {
                let removed = self.nodes.pop_at(old_index);
                if let Some(pending) = self.pending_node.take() {
                    self.nodes.push(pending);
                }
                removed
            })
    }

    pub fn get_pending(&self) -> Option<&Node<ID, V>> {
        self.pending_node.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        kbucket::{key::BinaryKey, BinaryID, Bucket, InsertResult},
        peer::PeerNode,
    };

    impl<ID: BinaryID, V> Bucket<ID, V> {
        pub fn last_id(&self) -> Option<&BinaryKey> {
            self.nodes.last().map(|n| n.id().as_binary())
        }
    }
    impl<ID: BinaryID, V> Bucket<ID, V> {
        pub fn least_used_id(&self) -> Option<&BinaryKey> {
            self.nodes.first().map(|n| n.id().as_binary())
        }
    }

    #[test]
    fn test_lru() {
        let mut bucket = Bucket::new();
        let node1 = PeerNode::from_address("192.168.1.1:8080".to_string());
        let id_node1 = node1.id().as_binary().clone();
        let node1_copy = PeerNode::from_address("192.168.1.1:8080".to_string());
        assert_eq!(InsertResult::Inserted, bucket.insert(node1));
        assert_eq!(InsertResult::Updated, bucket.insert(node1_copy));
        assert_eq!(Some(&id_node1), bucket.last_id());
        let node2 = PeerNode::from_address("192.168.1.2:8080".to_string());
        let id_node2 = node2.id().as_binary().clone();
        assert_eq!(InsertResult::Inserted, bucket.insert(node2));
        assert_eq!(Some(&id_node2), bucket.last_id());
        assert_eq!(Some(&id_node1), bucket.least_used_id());
        assert_eq!(
            InsertResult::Updated,
            bucket.insert(PeerNode::from_address("192.168.1.1:8080".to_string()))
        );
        assert_eq!(Some(&id_node1), bucket.last_id());
        assert_eq!(Some(&id_node2), bucket.least_used_id());
        let a = bucket.remove_id(&id_node2);
        assert_eq!(&id_node2, a.unwrap().id().as_binary());
        assert_eq!(Some(&id_node1), bucket.last_id());
        assert_eq!(Some(&id_node1), bucket.least_used_id());
        for i in 2..21 {
            assert_eq!(
                InsertResult::Inserted,
                bucket.insert(PeerNode::from_address(format!("192.168.1.{}:8080", i)))
            );
        }
        let pending = PeerNode::from_address("192.168.1.21:8080".to_string());
        let pending_id = pending.id().as_binary().clone();
        match bucket.insert(pending) {
            InsertResult::Pending(pending) => assert_eq!(pending.id().as_binary(), &pending_id),
            _ => assert!(false),
        }
        assert_eq!(&pending_id, bucket.get_pending().unwrap().id().as_binary());
        let lease_used_id = *bucket.least_used_id().clone().unwrap();
        let removed = bucket.remove_id(&lease_used_id);
        assert_eq!(removed.unwrap().id().as_binary(), &id_node1);
        assert!(bucket.get_pending().is_none());
        assert_eq!(bucket.last_id(), Some(&pending_id));
    }
}

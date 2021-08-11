use crate::peer;
use std::time::Instant;

use super::key::BinaryID;
#[derive(Debug,PartialEq, Eq, PartialOrd, Ord)]
pub struct Node<TKey: BinaryID, TValue> {
    id: TKey,
    value: TValue,
    last_used: Option<Instant>,
}

impl<TKey: BinaryID, TValue> Node<TKey, TValue> {
    pub fn new(id: TKey, value: TValue) -> Node<TKey, TValue> {
        Node {
            id,
            value,
            last_used: None,
        }
    }

    pub fn calculate_distance(&self, other: &Node<TKey, TValue>) -> Option<usize> {
        self.id.calculate_distance(&other.id)
    }

    //maybe we can move this outside of node impl, nonce must be verified when a node is deserialized IMHO
    pub fn is_id_valid(&self) -> bool {
        peer::verify_nonce(self.id.as_binary(), self.id.nonce())
    }

    pub fn id(&self) -> &TKey {
        &self.id
    }
}

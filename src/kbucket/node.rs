use std::time::{Duration, Instant};

use super::key::BinaryID;
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Node<TValue> {
    id: BinaryID,
    value: TValue,
    pub(super) eviction_status: NodeEvictionStatus,
    pub(super) seen_at: Instant,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeEvictionStatus {
    None,
    Requested(Instant),
}

impl<TValue> Node<TValue> {
    pub fn new(id: BinaryID, value: TValue) -> Self {
        Node {
            id,
            value,
            seen_at: Instant::now(),
            eviction_status: NodeEvictionStatus::None,
        }
    }

    pub fn calculate_distance(&self, other: &Node<TValue>) -> Option<usize> {
        self.id.calculate_distance(&other.id)
    }

    //maybe we can move this outside of node impl, nonce must be verified when a node is deserialized IMHO
    pub fn is_id_valid(&self) -> bool {
        self.id.verify_nonce()
    }

    pub fn id(&self) -> &BinaryID {
        &self.id
    }

    pub fn value(&self) -> &TValue {
        &self.value
    }

    pub(super) fn refresh(&mut self) {
        self.eviction_status = NodeEvictionStatus::None;
        self.seen_at = Instant::now();
    }

    pub(super) fn flag_for_check(&mut self) {
        self.eviction_status = NodeEvictionStatus::Requested(Instant::now());
    }

    pub(super) fn is_alive(&self, duration: Duration) -> bool {
        self.seen_at.elapsed() < duration
    }
}

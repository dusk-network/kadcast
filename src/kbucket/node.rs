use std::time::{Duration, Instant};

use crate::utils;

use super::key::BinaryID;
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Node<TKey: BinaryID, TValue> {
    id: TKey,
    value: TValue,
    pub(super) eviction_status: NodeEvictionStatus,
    pub(super) seen_at: Instant,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeEvictionStatus {
    None,
    Requested(Instant),
}

impl<TKey: BinaryID, TValue> Node<TKey, TValue> {
    pub fn new(id: TKey, value: TValue) -> Self {
        Node {
            id,
            value,
            seen_at: Instant::now(),
            eviction_status: NodeEvictionStatus::None,
        }
    }

    pub fn calculate_distance(&self, other: &Node<TKey, TValue>) -> Option<usize> {
        self.id.calculate_distance(&other.id)
    }

    //maybe we can move this outside of node impl, nonce must be verified when a node is deserialized IMHO
    pub fn is_id_valid(&self) -> bool {
        utils::verify_nonce(self.id.as_binary(), self.id.nonce())
    }

    pub fn id(&self) -> &TKey {
        &self.id
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

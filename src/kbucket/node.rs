// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::time::{Duration, Instant};

use super::key::BinaryID;
use super::BucketHeight;

/// A struct representing a node in the network with an associated ID, value,
/// and eviction status.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Node<TValue> {
    id: BinaryID,
    value: TValue,
    pub(super) eviction_status: NodeEvictionStatus,
    pub(super) seen_at: Instant,
    pub(crate) network_id: u8,
}

/// An enumeration representing the eviction status of a node.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeEvictionStatus {
    None,
    Requested(Instant),
}

impl<TValue> Node<TValue> {
    /// Creates a new node with the provided ID, value, and network ID.
    pub fn new(id: BinaryID, value: TValue, network_id: u8) -> Self {
        Node {
            id,
            value,
            seen_at: Instant::now(),
            eviction_status: NodeEvictionStatus::None,
            network_id,
        }
    }

    /// Calculates the distance between this node and another node in the
    /// network.
    pub fn calculate_distance(
        &self,
        other: &Node<TValue>,
    ) -> Option<BucketHeight> {
        self.id.calculate_distance(other.id.as_binary())
    }

    /// Returns the ID of the node.
    pub fn id(&self) -> &BinaryID {
        &self.id
    }

    /// Returns the value associated with the node.
    pub fn value(&self) -> &TValue {
        &self.value
    }

    /// Refreshes the last seen time and clears the eviction status for the
    /// node.
    pub(super) fn refresh(&mut self) {
        self.eviction_status = NodeEvictionStatus::None;
        self.seen_at = Instant::now();
    }

    /// Flags the node for eviction check by updating its eviction status.
    pub(super) fn flag_for_check(&mut self) {
        self.eviction_status = NodeEvictionStatus::Requested(Instant::now());
    }

    /// Checks if the node has been seen within a specified time duration.
    ///
    /// Returns `true` if the node's last seen time is within the given
    /// `duration`, indicating that it is still active and reachable in the
    /// network.
    pub(super) fn is_alive(&self, duration: Duration) -> bool {
        self.seen_at.elapsed() < duration
    }
}

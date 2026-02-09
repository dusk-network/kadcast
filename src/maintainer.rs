// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;

use semver::Version;
use tokio::sync::mpsc::Sender;
use tracing::{error, info};

use crate::encoding::message::{Header, Message};
use crate::kbucket::Tree;
use crate::peer::PeerInfo;
use crate::transport::MessageBeanOut;
use crate::{K_ALPHA, RwLock};

pub(crate) struct TableMaintainer {
    bootstrapping_nodes: Vec<String>,
    ktable: RwLock<Tree<PeerInfo>>,
    outbound_sender: Sender<MessageBeanOut>,
    my_ip: SocketAddr,
    header: Header,
    version: Version,
}

impl TableMaintainer {
    pub fn start(
        bootstrapping_nodes: Vec<String>,
        ktable: RwLock<Tree<PeerInfo>>,
        outbound_sender: Sender<MessageBeanOut>,
        idle_time: Duration,
        min_peers: usize,
        version: Version,
    ) {
        tokio::spawn(async move {
            let my_ip = *ktable.read().await.root().value().address();
            let header = ktable.read().await.root().to_header();

            let maintainer = Self {
                bootstrapping_nodes,
                ktable,
                outbound_sender,
                my_ip,
                header,
                version,
            };
            maintainer.monitor_buckets(idle_time, min_peers).await;
        });
    }

    /// Check if the peer need to contact the bootstrappers in order to join the
    /// network
    async fn need_bootstrappers(&self, min_peers: usize) -> bool {
        self.ktable.read().await.alive_nodes().count() < min_peers
    }

    /// Return a vector containing the Socket Addresses bound to the provided
    /// nodes
    fn bootstrapping_nodes_addr(&self) -> Vec<SocketAddr> {
        self.bootstrapping_nodes
            .iter()
            .flat_map(|boot| {
                boot.to_socket_addrs().unwrap_or_else(|e| {
                    error!("Unable to resolve domain for {} - {}", boot, e);
                    vec![].into_iter()
                })
            })
            .filter(|socket| socket != &self.my_ip)
            .collect()
    }

    /// Try to contact the bootstrappers node until no needed anymore
    async fn contact_bootstrappers(&self, min_peers: usize) {
        while self.need_bootstrappers(min_peers).await {
            info!("TableMaintainer::contact_bootstrappers");
            let bootstrapping_nodes_addr = self.bootstrapping_nodes_addr();
            let binary_key = self.header.binary_id().as_binary();
            let find_nodes = Message::FindNodes(
                self.header,
                self.version.clone(),
                *binary_key,
            );
            self.send((find_nodes, bootstrapping_nodes_addr)).await;
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }

    async fn send(&self, message: MessageBeanOut) {
        self.outbound_sender
            .send(message)
            .await
            .unwrap_or_else(|e| {
                error!("Unable to send message from maintainer {e}")
            })
    }

    /// This is the main function of this utility class. It's responsible to:
    /// 1. Contact bootstrappers (if needed)
    /// 2. Find new node for idle buckets
    /// 3. Remove idles nodes from buckets
    async fn monitor_buckets(&self, idle_time: Duration, min_peers: usize) {
        info!("TableMaintainer::monitor_buckets started");
        loop {
            self.contact_bootstrappers(min_peers).await;
            info!("TableMaintainer::monitor_buckets back to sleep");

            tokio::time::sleep(idle_time).await;

            info!("TableMaintainer::monitor_buckets woke up");
            self.find_new_nodes().await;

            info!("TableMaintainer::monitor_buckets removing idle nodes");
            self.ping_and_remove_idles().await;
        }
    }

    /// Search for idle buckets (no message received) and try to contact some of
    /// the belonging nodes
    async fn ping_and_remove_idles(&self) {
        let idles = self
            .ktable
            .read()
            .await
            .idle_nodes()
            .map(|n| *n.value().address())
            .collect();
        self.send((Message::Ping(self.header, self.version.clone()), idles))
            .await;
        self.ktable.write().await.remove_idle_nodes();
    }

    /// Searches for idle or empty buckets (those without received messages) in
    /// the routing table and requests information about the nodes in these
    /// buckets from active peers.
    ///
    /// For each identified idle or empty bucket, it calculates a target binary
    /// key using the `get_at_distance` method, which flips a specific bit
    /// in the node's binary identifier based on the given distance. This
    /// generates a new target key that is used to search for additional
    /// nodes.
    ///
    /// A set of active peers, up to `K_ALPHA`, is gathered from the current
    /// routing table and combined with the bootstrapping nodes to form the
    /// list of peers to contact.
    ///
    /// The purpose of this method is to keep the routing table active and up to
    /// date by finding new peers whenever buckets are empty or nodes become
    /// unresponsive.
    async fn find_new_nodes(&self) {
        let table_lock_read = self.ktable.read().await;
        let buckets_to_refresh = table_lock_read.idle_or_empty_height();

        let alive_peers = table_lock_read
            .alive_nodes()
            .map(|n| n.as_peer_info().to_socket_address())
            .take(K_ALPHA)
            .chain(self.bootstrapping_nodes_addr().into_iter())
            .collect::<Vec<_>>();

        for bucket_h in buckets_to_refresh {
            let target = self.header.binary_id().get_at_distance(bucket_h);
            let msg =
                Message::FindNodes(self.header, self.version.clone(), target);
            self.send((msg, alive_peers.clone())).await;
        }
    }
}

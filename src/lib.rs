// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::{convert::TryInto, net::SocketAddr, sync::Arc, time::Duration};

use encoding::{message::Message, payload::BroadcastPayload};
use handling::MessageHandler;
use itertools::Itertools;
use kbucket::{Tree, TreeBuilder};
use mantainer::TableMantainer;
use peer::{PeerInfo, PeerNode};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::RwLock;
use tokio::task;
use tracing::info;
use transport::{MessageBeanOut, WireNetwork};

mod encoding;
mod handling;
mod kbucket;
mod mantainer;
mod peer;
mod transport;

// Max amount of nodes a bucket should contain
const K_K: usize = 20;
const K_ID_LEN_BYTES: usize = 16;
const K_NONCE_LEN: usize = 4;
const K_DIFF_MIN_BIT: usize = 8;
const K_DIFF_PRODUCED_BIT: usize = 8;
const MAX_DATAGRAM_SIZE: usize = 65_507;

//Redundacy factor for lookup
const K_ALPHA: usize = 3;
//Redundacy factor for broadcast
const K_BETA: usize = 3;

const K_CHUNK_SIZE: u16 = 1024;

/// Default value while which a node is considered alive (no eviction will be
/// requested)
pub const BUCKET_DEFAULT_NODE_TTL_MILLIS: u64 = 30000;

/// Default value after while a node can be evicted if requested
pub const BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS: u64 = 5000;

/// Default value after while a bucket is considered idle
pub const BUCKET_DEFAULT_TTL_SECS: u64 = 60 * 60;

/// Struct representing the Kadcast Network
pub struct Server {
    outbound_sender: Sender<MessageBeanOut>,
    ktable: Arc<RwLock<Tree<PeerInfo>>>,
}

impl Server {
    fn new(
        public_ip: String,
        bootstrapping_nodes: Vec<String>,
        on_message: fn(Vec<u8>),
        tree: Tree<PeerInfo>,
    ) -> Self {
        let (inbound_channel_tx, inbound_channel_rx) = mpsc::channel(32);
        let (outbound_channel_tx, outbound_channel_rx) = mpsc::channel(32);
        let (notification_channel_tx, listener_channel_rx) = mpsc::channel(32);

        let table = Arc::new(RwLock::new(tree));
        let server = Server {
            outbound_sender: outbound_channel_tx.clone(),
            ktable: table.clone(),
        };
        MessageHandler::start(
            table.clone(),
            inbound_channel_rx,
            outbound_channel_tx.clone(),
            notification_channel_tx,
        );
        tokio::spawn(WireNetwork::start(
            inbound_channel_tx,
            public_ip,
            outbound_channel_rx,
        ));
        tokio::spawn(TableMantainer::start(
            bootstrapping_nodes,
            table,
            outbound_channel_tx,
        ));
        task::spawn(Server::notifier(listener_channel_rx, on_message));
        server
    }

    async fn notifier(
        mut listener_channel_rx: Receiver<Vec<u8>>,
        on_message: fn(Vec<u8>),
    ) {
        while let Some(message) = listener_channel_rx.recv().await {
            on_message(message);
        }
    }

    #[doc(hidden)]
    pub async fn report(&self) {
        let table_read = self.ktable.read().await;
        table_read.all_sorted().for_each(|(h, nodes)| {
            info!(
                "H: {} - Nodes {}",
                h,
                nodes.map(|p| p.value().address()).join(",")
            );
        });
    }

    /// Broadcast a message to the network.
    ///
    /// Note:
    /// The function returns just after the message is put on the internal queue
    /// system. It **does not guarantee** message is broadcasted in the
    /// meanwhile
    pub async fn broadcast(&self, message: &[u8]) {
        if message.is_empty() {
            return;
        }
        for (height, nodes) in self.ktable.read().await.extract(None) {
            let msg = Message::Broadcast(
                self.ktable.read().await.root().as_header(),
                BroadcastPayload {
                    height: height.try_into().unwrap(),
                    gossip_frame: message.to_vec(), //FIX_ME: avoid clone
                },
            );
            let targets: Vec<SocketAddr> =
                nodes.map(|node| *node.value().address()).collect();
            let _ = self.outbound_sender.send((msg, targets)).await;
        }
    }

    /// Instantiate a [ServerBuilder].
    ///
    /// * `public_ip` - String representing the public `SocketAddress` where
    ///   `Server` is reachable. No domain name allowed.
    /// * `bootstrapping_nodes` - List of known bootstrapping kadcast nodes. It
    ///   accepts same representation of `public_ip` but with domain names
    ///   allowed
    /// * `on_message` - Function called each time a broadcasted message is
    ///   received from the network
    pub fn builder(
        public_ip: String,
        bootstrapping_nodes: Vec<String>,
        on_message: fn(Vec<u8>),
    ) -> ServerBuilder {
        ServerBuilder::new(public_ip, bootstrapping_nodes, on_message)
    }
}

/// Kadcast [Server] builder
///
/// Builder for Kadcast [Server], allows to instantiate a [Server] and to tweak
/// his parameters
pub struct ServerBuilder {
    node_ttl: Duration,
    node_evict_after: Duration,
    bucket_ttl: Duration,
    public_ip: String,
    bootstrapping_nodes: Vec<String>,
    on_message: fn(Vec<u8>),
}

impl ServerBuilder {
    /// Set duration while which a node is considered alive (no eviction will be
    /// requested).
    ///
    /// Default value [BUCKET_DEFAULT_NODE_TTL_MILLIS]
    pub fn with_node_ttl(mut self, node_ttl: Duration) -> ServerBuilder {
        self.node_ttl = node_ttl;
        self
    }

    /// Set duration after while a bucket is considered idle
    ///
    /// Default value [BUCKET_DEFAULT_TTL_SECS]
    pub fn with_bucket_ttl(mut self, bucket_ttl: Duration) -> ServerBuilder {
        self.bucket_ttl = bucket_ttl;
        self
    }

    /// Set duration after while a node can be evicted if requested
    ///
    /// Default value [BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS]
    pub fn with_node_evict_after(
        mut self,
        node_evict_after: Duration,
    ) -> ServerBuilder {
        self.node_evict_after = node_evict_after;
        self
    }

    fn new(
        public_ip: String,
        bootstrapping_nodes: Vec<String>,
        on_message: fn(Vec<u8>),
    ) -> ServerBuilder {
        ServerBuilder {
            public_ip,
            bootstrapping_nodes,
            on_message,

            node_evict_after: Duration::from_millis(
                BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS,
            ),
            node_ttl: Duration::from_millis(BUCKET_DEFAULT_NODE_TTL_MILLIS),
            bucket_ttl: Duration::from_secs(BUCKET_DEFAULT_TTL_SECS),
        }
    }

    /// Builds the [Server]
    pub fn build(self) -> Server {
        let tree =
            TreeBuilder::new(PeerNode::from_address(&self.public_ip[..]))
                .with_node_evict_after(self.node_evict_after)
                .with_node_ttl(self.node_ttl)
                .with_bucket_ttl(self.bucket_ttl)
                .build();
        Server::new(
            self.public_ip,
            self.bootstrapping_nodes,
            self.on_message,
            tree,
        )
    }
}

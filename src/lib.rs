// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::net::AddrParseError;
use std::net::SocketAddr;
use std::time::Instant;

use config::Config;
use encoding::message::{Header, Message};
use encoding::payload::BroadcastPayload;
use handling::MessageHandler;
pub use handling::MessageInfo;
use itertools::Itertools;
use kbucket::MAX_BUCKET_HEIGHT;
use kbucket::{BucketHeight, Tree};
use maintainer::TableMaintainer;
use peer::{PeerInfo, PeerNode};
use rand::prelude::IteratorRandom;
pub(crate) use rwlock::RwLock;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task;
use tracing::warn;
use tracing::{error, info};
use transport::{MessageBeanOut, WireNetwork};

pub mod config;
mod encoding;
mod handling;
mod kbucket;
mod maintainer;
mod peer;
mod rwlock;
pub mod transport;

// Max amount of nodes a bucket should contain
const DEFAULT_K_K: usize = 20;
const K_K: usize = get_k_k();

const K_ID_LEN_BYTES: usize = 16;
const K_NONCE_LEN: usize = 4;
const K_DIFF_MIN_BIT: usize = 8;
const K_DIFF_PRODUCED_BIT: usize = 8;

const fn get_k_k() -> usize {
    match option_env!("KADCAST_K") {
        Some(v) => match konst::primitive::parse_usize(v) {
            Ok(e) => e,
            Err(_) => DEFAULT_K_K,
        },
        None => DEFAULT_K_K,
    }
}

// Redundacy factor for lookup
const K_ALPHA: usize = 3;
// Redundacy factor for broadcast
const K_BETA: usize = 3;

/// Struct representing the Kadcast Network Peer
pub struct Peer {
    outbound_sender: Sender<MessageBeanOut>,
    ktable: RwLock<Tree<PeerInfo>>,
    header: Header,
    blocklist: RwLock<HashSet<SocketAddr>>,
}

/// The [NetworkListen] trait receives notifications whenever a broadcasted
/// message is received from the network.
pub trait NetworkListen: Send {
    fn on_message(&self, message: Vec<u8>, metadata: MessageInfo);
}

impl Peer {
    /// Create a [Peer].
    ///
    /// * `config` - The [Config] used to create the Peer
    /// * `listener` - The [NetworkListen] impl notified each time a broadcasted
    ///   message is received from the network
    pub fn new<L: NetworkListen + 'static>(
        config: Config,
        listener: L,
    ) -> Result<Self, AddrParseError> {
        let network_id = config.kadcast_id.unwrap_or_default();
        let tree = Tree::new(
            PeerNode::generate(&config.public_address[..], network_id)?,
            config.bucket,
        );

        let (inbound_channel_tx, inbound_channel_rx) =
            mpsc::channel(config.channel_size);
        let (outbound_channel_tx, outbound_channel_rx) =
            mpsc::channel(config.channel_size);
        let (notification_channel_tx, listener_channel_rx) =
            mpsc::channel(config.channel_size);

        let header = tree.root().to_header();
        let table = rwlock::new(tree);
        let blocklist = rwlock::new(HashSet::new());
        let peer = Peer {
            outbound_sender: outbound_channel_tx.clone(),
            ktable: table.clone(),
            header,
            blocklist: blocklist.clone(),
        };
        let nodes = config.bootstrapping_nodes.clone();
        let idle_time = config.bucket.bucket_ttl;
        let min_peers = config.bucket.min_peers;
        let version =
            semver::Version::parse(&config.version).expect("Invalid version");

        MessageHandler::start(
            table.clone(),
            inbound_channel_rx,
            outbound_channel_tx.clone(),
            notification_channel_tx,
            &config,
        );
        WireNetwork::start(
            inbound_channel_tx,
            outbound_channel_rx,
            config,
            blocklist,
        );
        TableMaintainer::start(
            nodes,
            table,
            outbound_channel_tx,
            idle_time,
            min_peers,
            version,
        );
        task::spawn(Peer::notifier(listener_channel_rx, listener));
        Ok(peer)
    }

    async fn notifier(
        mut listener_channel_rx: Receiver<(Vec<u8>, MessageInfo)>,
        listener: impl NetworkListen,
    ) {
        while let Some(notif) = listener_channel_rx.recv().await {
            listener.on_message(notif.0, notif.1);
        }
    }

    /// Return the [SocketAddr] of a set of random active nodes.
    ///
    /// * `amount` - The max amount of nodes to return
    pub async fn alive_nodes(&self, amount: usize) -> Vec<SocketAddr> {
        let table_read = self.ktable.read().await;

        // If the `rng` is generated between the `await`, it leads into "the
        // trait `std::marker::Send` is not implemented for
        // `Rc<UnsafeCell<ReseedingRng<rand_chacha::chacha::ChaCha12Core,
        // OsRng>>>`" when used outside this crate
        let rng = &mut rand::thread_rng();
        table_read
            .alive_nodes()
            .map(|i| i.as_peer_info().to_socket_address())
            .choose_multiple(rng, amount)
    }

    #[doc(hidden)]
    pub async fn report(&self) {
        let table_read = self.ktable.read().await;
        /*
        The usage of `info!` macro can potentially raise a compilation error
        depending of which `tracing` crate features are used.

        Eg: if you use the `log` feature (or if you have any dependency which
        enables it), the `info` macro perform some `move` internally which
        conflicts with the `move` performed by the `map` method.

        Refactoring this way, we are sure there will be only one `move`
        independently to which features are you using

        See also: https://github.com/dusk-network/kadcast/issues/60
        */
        table_read.all_sorted().for_each(|(h, nodes)| {
            let nodes_joined = nodes.map(|p| p.value().address()).join(",");
            info!("H: {h} - Nodes {nodes_joined}");
        });
    }

    /// Returns the current routing table.
    ///
    /// # Returns
    /// A `BTreeMap<u8, Vec<(SocketAddr, Instant)>>` where each key is the
    /// bucket height and each value is a vector of tuples containing a
    /// node's address and last seen time.
    pub async fn to_route_table(
        &self,
    ) -> BTreeMap<u8, Vec<(SocketAddr, Instant)>> {
        let mut route_table = BTreeMap::new();

        let table_read = self.ktable.read().await;
        table_read.buckets().for_each(|(h, nodes)| {
            let nodes = nodes
                .map(|p| (*p.value().address(), *p.seen_at()))
                .collect::<Vec<_>>();
            route_table.insert(h, nodes);
        });

        route_table
    }

    /// Broadcast a message to the network
    ///
    /// # Arguments
    ///
    /// * `message` - Byte array containing the message to be broadcasted
    /// * `height` - (Optional) Overrides default Kadcast broadcast height
    ///
    /// Note:
    /// The function returns just after the message is put on the internal queue
    /// system. It **does not guarantee** the message will be broadcasted
    pub async fn broadcast(
        &self,
        message: &[u8],
        height: Option<BucketHeight>,
    ) {
        if message.is_empty() {
            error!("Message empty");
            return;
        }

        for i in self.extract(message, height).await {
            self.outbound_sender.send(i).await.unwrap_or_else(|e| {
                error!("Unable to send from broadcast {e}")
            });
        }
    }

    async fn extract(
        &self,
        message: &[u8],
        height: Option<BucketHeight>,
    ) -> Vec<(Message, Vec<SocketAddr>)> {
        const LAST_BUCKET_IDX: u8 = MAX_BUCKET_HEIGHT as u8 - 1;
        let ktable = self.ktable.read().await;
        if height.is_none() && ktable.bucket_size(LAST_BUCKET_IDX) == 0 {
            warn!("Broadcasting a new message with empty bucket height {LAST_BUCKET_IDX}")
        }
        ktable
            .extract(height)
            .map(|(height, nodes)| {
                let msg = Message::broadcast(
                    self.header,
                    BroadcastPayload {
                        height,
                        gossip_frame: message.to_vec(),
                    },
                );
                let targets =
                    nodes.map(|node| *node.value().address()).collect();
                (msg, targets)
            })
            .collect()
    }

    /// Send a message to a peer in the network
    ///
    /// # Arguments
    ///
    /// * `message` - Byte array containing the message to be sent
    /// * `target` - Receiver address
    ///
    /// Note:
    /// The function returns just after the message is put on the internal queue
    /// system. It **does not guarantee** the message will be broadcasted
    pub async fn send(&self, message: &[u8], target: SocketAddr) {
        self.send_to_peers(message, vec![target]).await
    }

    /// Send a message to multiple peers in the network
    ///
    /// # Arguments
    ///
    /// * `message` - Byte array containing the message to be sent
    /// * `targets` - Vector of receiver addresses (`Vec<SocketAddr>`)
    ///
    /// Note:
    /// The function returns just after the message is put on the internal queue
    /// system. It **does not guarantee** the message will be broadcasted to
    /// all.
    pub async fn send_to_peers(
        &self,
        message: &[u8],
        targets: Vec<SocketAddr>,
    ) {
        if message.is_empty() {
            return;
        }
        // We use the Broadcast message type while setting height to 0
        // to prevent further propagation at the receiver
        let msg = Message::broadcast(
            self.header,
            BroadcastPayload {
                height: 0,
                gossip_frame: message.to_vec(),
            },
        );
        self.outbound_sender
            .send((msg, targets))
            .await
            .unwrap_or_else(|e| error!("Unable to send from send method {e}"));
    }

    /// Blocks a network source and removes it from the routing table.
    ///
    /// # Arguments
    ///
    /// * `source` - The address of the network source to be blocked.
    ///
    /// This method blocks a network source by adding its address to the
    /// blocklist and subsequently removes the corresponding peer from the
    /// routing table. This action prevents further communication with the
    /// blocked source.
    pub async fn block_source(&self, source: SocketAddr) {
        self.blocklist.write().await.insert(source);
        let binary_key = PeerNode::compute_id(&source.ip(), source.port());
        self.ktable.write().await.remove_peer(&binary_key);
    }
}

#[cfg(test)]
mod tests {

    pub type Result<T> = core::result::Result<T, Box<dyn std::error::Error>>;
}

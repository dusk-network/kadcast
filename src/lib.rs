// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::{convert::TryInto, net::SocketAddr, time::Duration};

use config::Config;
use encoding::message::Header;
use encoding::{message::Message, payload::BroadcastPayload};
use handling::MessageHandler;
pub use handling::MessageInfo;
use itertools::Itertools;
use kbucket::Tree;
use mantainer::TableMantainer;
use peer::{PeerInfo, PeerNode};
pub(crate) use rwlock::RwLock;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task;
use tracing::{error, info};
use transport::{MessageBeanOut, WireNetwork};

pub mod config;
mod encoding;
mod handling;
mod kbucket;
mod mantainer;
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
}

/// [NetworkListen] is notified each time a broadcasted
/// message is received from the network
pub trait NetworkListen: Send {
    fn on_message(&self, message: Vec<u8>, metadata: MessageInfo);
}

impl Peer {
    pub fn new<L: NetworkListen + 'static>(
        config: Config,
        listener: L,
    ) -> Self {
        let tree = Tree::new(
            PeerNode::from_address(&config.public_address[..]),
            config.bucket,
        );

        let (inbound_channel_tx, inbound_channel_rx) =
            mpsc::channel(config.channel_size);
        let (outbound_channel_tx, outbound_channel_rx) =
            mpsc::channel(config.channel_size);
        let (notification_channel_tx, listener_channel_rx) =
            mpsc::channel(config.channel_size);

        let header = tree.root().as_header();
        let table = RwLock::new(tree, Duration::from_secs(1));
        let peer = Peer {
            outbound_sender: outbound_channel_tx.clone(),
            ktable: table.clone(),
            header,
        };
        let bootstrapping_nodes = config.bootstrapping_nodes.clone();
        MessageHandler::start(
            table.clone(),
            inbound_channel_rx,
            outbound_channel_tx.clone(),
            notification_channel_tx,
            &config,
        );
        WireNetwork::start(inbound_channel_tx, outbound_channel_rx, config);
        tokio::spawn(TableMantainer::start(
            bootstrapping_nodes,
            table,
            outbound_channel_tx,
        ));
        task::spawn(Peer::notifier(listener_channel_rx, listener));
        peer
    }

    async fn notifier(
        mut listener_channel_rx: Receiver<(Vec<u8>, MessageInfo)>,
        listener: impl NetworkListen,
    ) {
        while let Some(notif) = listener_channel_rx.recv().await {
            listener.on_message(notif.0, notif.1);
        }
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
            info!("H: {} - Nodes {}", h, nodes_joined);
        });
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
    pub async fn broadcast(&self, message: &[u8], height: Option<usize>) {
        if message.is_empty() {
            error!("Message empty");
            return;
        }

        let tosend: Vec<(Message, Vec<SocketAddr>)> = self
            .ktable
            .read()
            .await
            .extract(height)
            .map(|(h, nodes)| {
                let msg = Message::Broadcast(
                    self.header,
                    BroadcastPayload {
                        height: h.try_into().unwrap(),
                        gossip_frame: message.to_vec(), //FIX_ME: avoid clone
                    },
                );
                let targets: Vec<SocketAddr> =
                    nodes.map(|node| *node.value().address()).collect();
                (msg, targets)
            })
            .collect();

        for i in tosend {
            self.outbound_sender.send(i).await.unwrap_or_else(|e| {
                error!("Unable to send from broadcast {}", e)
            });
        }
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
        if message.is_empty() {
            return;
        }
        // We use the Broadcast message type while setting height to 0
        // to prevent further propagation at the receiver
        let msg = Message::Broadcast(
            self.header,
            BroadcastPayload {
                height: 0,
                gossip_frame: message.to_vec(), //FIX_ME: avoid clone
            },
        );
        let targets = vec![target];
        self.outbound_sender
            .send((msg, targets))
            .await
            .unwrap_or_else(|e| {
                error!("Unable to send from send method {}", e)
            });
    }

    // /// Instantiate a [PeerBuilder].
    // ///
    // /// * `public_address` - public `SocketAddress` of the [Peer]. No domain
    // ///   name allowed
    // /// * `bootstrapping_nodes` - List of known bootstrapping kadcast nodes.
    // It ///   accepts the same representation of `public_address` but with
    // domain ///   names allowed
    // /// * `listener` - The [NetworkListen] impl notified each time a
    // broadcasted ///   message is received from the network
    // pub fn builder<L: NetworkListen>(
    //     public_address: String,
    //     bootstrapping_nodes: Vec<String>,
    //     listener: L,
    // ) -> PeerBuilder<L> {
    //     PeerBuilder::new(public_address, bootstrapping_nodes, listener)
    // }
}

// /// PeerBuilder instantiates a Kadcast [Peer].
// pub struct PeerBuilder<L: NetworkListen + 'static> {
//     node_ttl: Duration,
//     node_evict_after: Duration,
//     bucket_ttl: Duration,
//     auto_propagate: bool,
//     public_address: String,
//     listen_address: Option<String>,
//     bootstrapping_nodes: Vec<String>,
//     listener: L,
//     transport_conf: HashMap<String, String>,
//     channel_size: usize,
//     recursive_discovery: bool,
// }

// impl<L: NetworkListen + 'static> PeerBuilder<L> {
//     /// Sets the maximum duration for a node to be considered alive (no
// eviction     /// will be requested).
//     ///
//     /// Default value [BUCKET_DEFAULT_NODE_TTL_MILLIS]
//     pub fn with_node_ttl(mut self, node_ttl: Duration) -> PeerBuilder<L> {
//         self.node_ttl = node_ttl;
//         self
//     }

//     /// Set duration after which a bucket is considered idle
//     ///
//     /// Default value [BUCKET_DEFAULT_TTL_SECS]
//     pub fn with_bucket_ttl(mut self, bucket_ttl: Duration) -> PeerBuilder<L>
// {         self.bucket_ttl = bucket_ttl;
//         self
//     }

//     /// Set duration after which a node can be evicted if requested
//     ///
//     /// Default value [BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS]
//     pub fn with_node_evict_after(
//         mut self,
//         node_evict_after: Duration,
//     ) -> PeerBuilder<L> {
//         self.node_evict_after = node_evict_after;
//         self
//     }

//     /// Set the address listening to incoming connections
//     /// If not set, the public address will be used
//     ///
//     /// Default value NONE
//     pub fn with_listen_address(
//         mut self,
//         listen_address: Option<String>,
//     ) -> PeerBuilder<L> {
//         self.listen_address = listen_address;
//         self
//     }

//     pub fn transport_conf(&mut self) -> &mut HashMap<String, String> {
//         &mut self.transport_conf
//     }

//     /// Enable automatic propagation of incoming broadcast messages
//     ///
//     /// Default value [ENABLE_BROADCAST_PROPAGATION]
//     pub fn with_auto_propagate(
//         mut self,
//         auto_propagate: bool,
//     ) -> PeerBuilder<L> {
//         self.auto_propagate = auto_propagate;
//         self
//     }

//     /// Default value [DEFAULT_CHANNEL_SIZE]
//     pub fn with_channel_size(mut self, channel_size: usize) -> PeerBuilder<L>
// {         self.channel_size = channel_size;
//         self
//     }

//     /// Default value true
//     pub fn with_recursive_discovery(
//         mut self,
//         recursive_discovery: bool,
//     ) -> PeerBuilder<L> {
//         self.recursive_discovery = recursive_discovery;
//         self
//     }

//     fn new(
//         public_address: String,
//         bootstrapping_nodes: Vec<String>,
//         listener: L,
//     ) -> PeerBuilder<L> {
//         PeerBuilder {
//             public_address,
//             listen_address: None,
//             bootstrapping_nodes,
//             listener,
//             node_evict_after: Duration::from_millis(
//                 BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS,
//             ),
//             node_ttl: Duration::from_millis(BUCKET_DEFAULT_NODE_TTL_MILLIS),
//             bucket_ttl: Duration::from_secs(BUCKET_DEFAULT_TTL_SECS),
//             auto_propagate: ENABLE_BROADCAST_PROPAGATION,
//             transport_conf: transport::default_configuration(),
//             channel_size: DEFAULT_CHANNEL_SIZE,
//             recursive_discovery: true,
//         }
//     }

//     /// Builds the [Peer]
//     pub fn build(self) -> Peer {
//         let tree =
//
// TreeBuilder::new(PeerNode::from_address(&self.public_address[..]))
//                 .with_node_evict_after(self.node_evict_after)
//                 .with_node_ttl(self.node_ttl)
//                 .with_bucket_ttl(self.bucket_ttl)
//                 .build();
//         Peer::new(
//             self.listen_address.unwrap_or(self.public_address),
//             self.bootstrapping_nodes,
//             self.auto_propagate,
//             self.recursive_discovery,
//             self.channel_size,
//             self.listener,
//             tree,
//             self.transport_conf,
//         )
//     }
// }

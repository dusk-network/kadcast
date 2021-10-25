use std::{convert::TryInto, net::SocketAddr, sync::Arc, time::Duration};

use encoding::{message::Message, payload::BroadcastPayload};
use handling::MessageHandler;
use itertools::Itertools;
use kbucket::{Tree, TreeBuilder};
use mantainer::TableMantainer;
use peer::{PeerInfo, PeerNode};
use tokio::sync::mpsc::{self, Sender};
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

const K_CHUNK_SIZE: usize = 1024;

const BUCKET_DEFAULT_NODE_TTL_MILLIS: u64 = 30000;
const BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS: u64 = 5000;
const BUCKET_DEFAULT_TTL_SECS: u64 = 60 * 60;

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
        let (listener_channel_tx, mut listener_channel_rx) = mpsc::channel(32);

        let table = Arc::new(RwLock::new(tree));
        let server = Server {
            outbound_sender: outbound_channel_tx.clone(),
            ktable: table.clone(),
        };
        MessageHandler::start(
            table.clone(),
            inbound_channel_rx,
            outbound_channel_tx.clone(),
            listener_channel_tx,
        );
        tokio::spawn(async move {
            WireNetwork::start(&inbound_channel_tx, &public_ip, outbound_channel_rx).await;
        });
        tokio::spawn(async move {
            TableMantainer::start(bootstrapping_nodes, &table.clone(), outbound_channel_tx).await;
        });

        task::spawn(async move {
            while let Some(message) = listener_channel_rx.recv().await {
                on_message(message);
            }
        });
        server
    }

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

    pub async fn broadcast(&self, message: Vec<u8>) {
        if message.is_empty() {
            return;
        }
        for (height, nodes) in self.ktable.read().await.extract(None) {
            let msg = Message::Broadcast(
                self.ktable.read().await.root().as_header(),
                BroadcastPayload {
                    height: height.try_into().unwrap(),
                    gossip_frame: message.clone(), //FIX_ME: avoid clone
                },
            );
            let targets: Vec<SocketAddr> = nodes.map(|node| *node.value().address()).collect();
            let _ = self.outbound_sender.send((msg, targets)).await;
        }
    }

    pub fn builder(
        public_ip: String,
        bootstrapping_nodes: Vec<String>,
        on_message: fn(Vec<u8>),
    ) -> ServerBuilder {
        ServerBuilder::new(public_ip, bootstrapping_nodes, on_message)
    }
}

pub struct ServerBuilder {
    node_ttl: Duration,
    node_evict_after: Duration,
    bucket_ttl: Duration,
    public_ip: String,
    bootstrapping_nodes: Vec<String>,
    on_message: fn(Vec<u8>),
}

impl ServerBuilder {
    pub fn with_node_ttl(mut self, node_ttl: Duration) -> ServerBuilder {
        self.node_ttl = node_ttl;
        self
    }

    pub fn with_bucket_ttl(mut self, bucket_ttl: Duration) -> ServerBuilder {
        self.bucket_ttl = bucket_ttl;
        self
    }

    pub fn with_node_evict_after(mut self, node_evict_after: Duration) -> ServerBuilder {
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

            node_evict_after: Duration::from_millis(BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS),
            node_ttl: Duration::from_millis(BUCKET_DEFAULT_NODE_TTL_MILLIS),
            bucket_ttl: Duration::from_secs(BUCKET_DEFAULT_TTL_SECS),
        }
    }

    pub fn build(self) -> Server {
        let tree = TreeBuilder::new(PeerNode::from_address(&self.public_ip[..]))
            .with_node_evict_after(self.node_evict_after)
            .with_node_ttl(self.node_evict_after)
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

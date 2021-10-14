use std::{convert::TryInto, net::SocketAddr, sync::Arc};

use encoding::{message::Message, payload::BroadcastPayload};
use itertools::Itertools;
use kbucket::{Tree, TreeBuilder};
use mantainer::TableMantainer;
use peer::{PeerInfo, PeerNode};
use tokio::sync::{
    mpsc::{self, Sender},
    RwLock,
};
use tracing::info;
use transport::{MessageBeanOut, WireNetwork};

mod encoding;
pub mod kbucket;
mod mantainer;
mod peer;
mod transport;

// Max amount of nodes a bucket should contain
pub const K_K: usize = 20;
pub const K_ID_LEN_BYTES: usize = 16;
pub const K_NONCE_LEN: usize = 4;
pub const K_DIFF_MIN_BIT: usize = 8;
pub const K_DIFF_PRODUCED_BIT: usize = 8;
const MAX_DATAGRAM_SIZE: usize = 65_507;

//Redundacy factor for lookup
const K_ALPHA: usize = 3;
//Redundacy factor for broadcast
const K_BETA: usize = 3;

const K_CHUNK_SIZE: usize = 1024;

pub struct Server {
    // mantainer: TableMantainer,
    // network: WireNetwork,
    outbound_sender: Sender<MessageBeanOut>,
    ktable: Arc<RwLock<Tree<PeerInfo>>>,
}

impl Server {
    pub fn new(public_ip: String, bootstrapping_nodes: Vec<String>) -> Self {
        let (inbound_channel_tx, inbound_channel_rx) = mpsc::channel(32);
        let (outbound_channel_tx, outbound_channel_rx) = mpsc::channel(32);

        // let network = WireNetwork::new(public_ip, inbound_channel_tx);
        let tree = TreeBuilder::new(PeerNode::from_address(&public_ip)).build();
        let table = Arc::new(RwLock::new(tree));
        let mantainer = TableMantainer::new(bootstrapping_nodes, table.clone());
        let s = Server {
            // mantainer,
            outbound_sender: outbound_channel_tx.clone(),
            ktable: table,
        };
        tokio::spawn(async move {
            WireNetwork::start(&inbound_channel_tx, &public_ip, outbound_channel_rx).await;
        });
        tokio::spawn(async move {
            mantainer
                .start(inbound_channel_rx, outbound_channel_tx)
                .await;
        });
        s
    }

    pub async fn report(&self){
        self.ktable.read().await.all_sorted().for_each(|(h, nodes)|{
            info!("H: {} - Nodes {}", h, nodes.map(|p| p.value().address()).join(","));
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
}

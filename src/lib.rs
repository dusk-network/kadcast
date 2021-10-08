use std::{convert::TryInto, net::SocketAddr};

use encoding::{message::KadcastMessage, payload::BroadcastPayload};
use kbucket::TreeBuilder;
use mantainer::TableMantainer;
use peer::{PeerInfo, PeerNode};
use tokio::sync::mpsc::{self, Sender};
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

//Redundacy factor for lookup
const K_ALPHA: usize = 3;
//Redundacy factor for broadcast
const K_BETA: usize = 3;

const K_CHUNK_SIZE: usize = 1024;

pub struct KadcastServer {
    mantainer: TableMantainer,
    network: WireNetwork,
    outbound_sender: Sender<MessageBeanOut>,
}

impl KadcastServer {
    pub async fn bootstrap(self) {
        let a = self.mantainer.start();
        // let runner_out= self.network.listen_out();
        let runner = self.network.start();
        // let mn = self.mantainer;
        // mn.bootstrap();
        tokio::join!(a, runner);
    }

    pub fn broadcast(&self, message: Vec<u8>) {
        for (idx, peers) in self.mantainer.ktable().extract(None) {
            let msg = KadcastMessage::Broadcast(
                self.mantainer.ktable().root().as_header(),
                BroadcastPayload {
                    height: idx.try_into().unwrap(),
                    gossip_frame: message.clone(), //FIX_ME: avoid clone
                },
            );
            let targets: Vec<SocketAddr> = peers.map(|s| *s.value().address()).collect();
            // let targets: Vec<SocketAddr> = targets.iter().map(|&s| s.clone()).collect();
            self.outbound_sender
                .blocking_send((msg, targets))
                .unwrap_or_default(); //FIX_ME: handle correctly
        }
    }
}

pub struct KadcastServerBuilder {
    tree_builder: TreeBuilder<PeerInfo>,
    bootstrapping_nodes: Vec<String>,
    public_ip: String,
}

impl KadcastServerBuilder {
    pub fn new(public_ip: &str, bootstrapping_nodes: Vec<String>) -> Self {
        KadcastServerBuilder {
            tree_builder: TreeBuilder::new(PeerNode::from_address(public_ip)),
            bootstrapping_nodes,
            public_ip: public_ip.to_string(),
        }
    }

    pub fn tree_builder(self) -> TreeBuilder<PeerInfo> {
        self.tree_builder
    }

    pub fn build(self) -> KadcastServer {
        let (inbound_channel_tx, inbound_channel_rx) = mpsc::channel(32);
        let network = WireNetwork::new(&self.public_ip, inbound_channel_tx);
        KadcastServer {
            mantainer: TableMantainer::new(
                self.bootstrapping_nodes,
                self.tree_builder.build(),
                network.sender(),
                inbound_channel_rx,
            ),
            outbound_sender: network.sender(),
            network,
        }
    }
}

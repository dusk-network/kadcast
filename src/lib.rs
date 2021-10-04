use std::convert::TryInto;

use encoding::{message::KadcastMessage, payload::BroadcastPayload};
use kbucket::{Tree, TreeBuilder};
use mantainer::TableMantainer;
use peer::{PeerInfo, PeerNode};
use transport::WireNetwork;

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

pub struct KadcastProto<TListener: KadcastBroadcastListener> {
    listener: Option<TListener>, //FIX_ME: No need to use this pattern, better rely on https://docs.rs/tokio/1.12.0/tokio/sync/index.html#watch-channel
    mantainer: TableMantainer,
    network: WireNetwork,
}

impl<TListener: KadcastBroadcastListener> KadcastProto<TListener> {
    pub fn bootstrap(&mut self) {
        self.mantainer.bootstrap();
    }

    pub fn send_message(&self, message: Vec<u8>) {
        for (idx, peers) in self.mantainer.ktable().extract(None) {
            let msg = KadcastMessage::Broadcast(
                self.mantainer.ktable().root().as_header(),
                BroadcastPayload {
                    height: idx.try_into().unwrap(),
                    gossip_frame: message.clone(), //FIX_ME: avoid clone
                },
            );
            for ele in peers {
                self.network.send_message(&msg, ele.value().address())
            }
        }
    }
}

pub struct KadcastProtoBuilder<'a, TListener: KadcastBroadcastListener> {
    tree_builder: TreeBuilder<PeerInfo>,
    bootstrapping_nodes: Vec<String>,
    listener: Option<TListener>,
    public_ip: &'a str,
}

impl<'a, TListener: KadcastBroadcastListener> KadcastProtoBuilder<'a, TListener> {
    pub fn new(
        public_ip: &str,
        bootstrapping_nodes: Vec<String>,
    ) -> KadcastProtoBuilder<TListener> {
        KadcastProtoBuilder {
            tree_builder: TreeBuilder::new(PeerNode::from_address(public_ip)),
            bootstrapping_nodes: bootstrapping_nodes,
            listener: None,
            public_ip,
        }
    }

    pub fn set_broadcast_listener(mut self, listener: TListener) -> Self {
        self.listener = Some(listener);
        self
    }

    pub fn tree_builder(self) -> TreeBuilder<PeerInfo> {
        self.tree_builder
    }

    pub fn build(self) -> KadcastProto<TListener> {
        KadcastProto {
            mantainer: TableMantainer::new(self.bootstrapping_nodes, self.tree_builder.build()),
            listener: self.listener,
            network: WireNetwork::new(self.public_ip),
        }
    }
}

pub trait KadcastBroadcastListener {
    //this should handle onky BroadcastMessages
    fn on_broadcast(&self, message: Vec<u8>);
}

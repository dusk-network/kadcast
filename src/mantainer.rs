use std::{
    convert::TryInto,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use tokio::sync::{
    mpsc::{Receiver, Sender},
    RwLock,
};
use tracing::*;

use crate::{
    encoding::{
        message::Message,
        payload::{BroadcastPayload, NodePayload},
    },
    kbucket::Tree,
    peer::{PeerInfo, PeerNode},
    transport::{MessageBeanIn, MessageBeanOut},
};

pub(crate) struct TableMantainer {
    bootstrapping_nodes: Vec<String>,
    ktable: Arc<RwLock<Tree<PeerInfo>>>,
}

impl TableMantainer {
    async fn find_node(&self) -> Message {
        let table = self.ktable.read().await;
        let root = table.root();
        Message::FindNodes(
            root.as_header(),
            NodePayload {
                peers: vec![root.as_peer_info()],
            },
        )
    }

    pub async fn start(
        self,
        mut inbound_receiver: Receiver<MessageBeanIn>,
        outbound_sender: Sender<MessageBeanOut>,
        listener_sender: Sender<Vec<u8>>,
    ) {
        let find_node = self.find_node().await;
        let bootstrapping_nodes = self
            .bootstrapping_nodes
            .iter()
            .flat_map(|boot| {
                boot.to_socket_addrs()
                    .expect("Unable to resolve domain for {}")
            })
            .collect();
        outbound_sender
            .send((find_node, bootstrapping_nodes))
            .await
            .unwrap_or_else(|op| error!("Unable to send generic {:?}", op));
        while let Some((message, mut node_socket)) = inbound_receiver.recv().await {
            debug!("Mantainer received message {:?}", message);
            node_socket.set_port(message.header().sender_port);
            let node = PeerNode::from_socket(node_socket);
            let my_header = self.ktable.read().await.root().as_header();
            let id = *node.id();
            let mut table_write = self.ktable.write().await;
            if let Ok(result) = table_write.insert(node) {
                debug!("Written node in ktable: {:?}", &result);
                if let Some(pending) = result.pending() {
                    outbound_sender
                        .try_send((Message::Ping(my_header), vec![*pending.value().address()]))
                        .unwrap_or_else(|op| {
                            error!("Unable to send PING to pending node {:?}", op)
                        });
                }
                drop(result);
                drop(table_write);
                match message {
                    Message::Ping(_) => {
                        let pong = Message::Pong(my_header);
                        outbound_sender
                            .try_send((pong, vec![node_socket]))
                            .unwrap_or_else(|op| error!("Unable to send Pong {:?}", op));
                    }
                    Message::Pong(_) => {}
                    Message::FindNodes(_, _) => {
                        let table_read = self.ktable.read().await;
                        let nodes = Message::Nodes(
                            my_header,
                            NodePayload {
                                peers: table_read
                                    .closest_peers(&id)
                                    .map(|p| p.as_peer_info())
                                    .collect(),
                            },
                        );
                        outbound_sender
                            .try_send((nodes, vec![node_socket]))
                            .unwrap_or_else(|op| error!("Unable to send Nodes {:?}", op));
                    }
                    Message::Nodes(_, nodes) => {
                        if !nodes.peers.is_empty() {
                            let targets =
                                nodes.peers.iter().map(|n| n.to_socket_address()).collect();
                            outbound_sender
                                .try_send((Message::Ping(my_header), targets))
                                .unwrap_or_else(|op| error!("Unable to send PING {:?}", op));
                        }
                    }
                    Message::Broadcast(_, payload) => {
                        debug!("Received payload {:?}", payload);
                        listener_sender
                            .try_send(payload.gossip_frame.clone())
                            .unwrap_or_else(|op| error!("Unable to notify client {:?}", op));
                        let table_read = self.ktable.read().await;
                        if payload.height > 0 {
                            for (height, nodes) in
                                table_read.extract(Some((payload.height - 1).into()))
                            {
                                let msg = Message::Broadcast(
                                    my_header,
                                    BroadcastPayload {
                                        height: height.try_into().unwrap(),
                                        gossip_frame: payload.gossip_frame.clone(), //FIX_ME: avoid clone
                                    },
                                );
                                let targets: Vec<SocketAddr> =
                                    nodes.map(|node| *node.value().address()).collect();
                                outbound_sender
                                    .try_send((msg, targets))
                                    .unwrap_or_else(|op| {
                                        error!("Unable to send broadcast {:?}", op)
                                    });
                            }
                        }
                    }
                };
            } else {
                error!("Unable to insert node");
            }
        }
    }

    pub fn new(bootstrapping_nodes: Vec<String>, ktable: Arc<RwLock<Tree<PeerInfo>>>) -> Self {
        TableMantainer {
            bootstrapping_nodes,
            ktable,
        }
    }
}

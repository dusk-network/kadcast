use std::net::{SocketAddr, ToSocketAddrs};
use std::thread;
use std::time::Duration;
use std::{convert::TryInto, sync::Arc};

use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::RwLock;
use tracing::*;

use crate::encoding::message::{BroadcastPayload, Message, NodePayload};
use crate::kbucket::{Tree, BUCKET_DEFAULT_TTL_SECS};
use crate::peer::{PeerInfo, PeerNode};
use crate::transport::{MessageBeanIn, MessageBeanOut};

pub(crate) struct TableMantainer {
    bootstrapping_nodes: Vec<String>,
    ktable: Arc<RwLock<Tree<PeerInfo>>>,
}

impl TableMantainer {
    async fn find_node_message(&self) -> Message {
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
        inbound_receiver: Receiver<MessageBeanIn>,
        outbound_sender: Sender<MessageBeanOut>,
        listener_sender: Sender<Vec<u8>>,
    ) {
        let bootstrap = self.bootstrap(outbound_sender.clone());
        let inbound = self.handle_inbound_messages(
            inbound_receiver,
            outbound_sender.clone(),
            listener_sender,
        );
        let monitor = self.monitor_buckets(outbound_sender);
        tokio::join!(bootstrap, inbound, monitor);
    }

    async fn bootstrap(&self, outbound_sender: Sender<MessageBeanOut>) {
        outbound_sender
            .send((
                self.find_node_message().await,
                self.bootstrapping_nodes
                    .iter()
                    .flat_map(|boot| {
                        boot.to_socket_addrs()
                            .expect("Unable to resolve domain for {}")
                    })
                    .collect(),
            ))
            .await
            .unwrap_or_else(|op| error!("Unable to send generic {:?}", op));
    }

    async fn monitor_buckets(&self, outbound_sender: Sender<MessageBeanOut>) {
        loop {
            thread::sleep(Duration::from_secs(BUCKET_DEFAULT_TTL_SECS));
            let table_lock_read = self.ktable.read().await;
            let root = table_lock_read.root();
            table_lock_read.idle_buckets().for_each(|(_, nodes)| {
                outbound_sender
                    .try_send((
                        Message::FindNodes(
                            table_lock_read.root().as_header(),
                            NodePayload {
                                peers: vec![root.as_peer_info()],
                            },
                        ),
                        nodes.map(|node| *node.value().address()).collect(),
                    ))
                    .unwrap_or_else(|op| error!("Unable to send broadcast {:?}", op));
            });
        }
    }

    async fn handle_inbound_messages(
        &self,
        mut inbound_receiver: Receiver<MessageBeanIn>,
        outbound_sender: Sender<MessageBeanOut>,
        listener_sender: Sender<Vec<u8>>,
    ) {
        while let Some((message, mut node_socket)) = inbound_receiver.recv().await {
            debug!("Mantainer received message {:?}", message);
            node_socket.set_port(message.header().sender_port);
            let node = PeerNode::from_socket(node_socket);
            let my_header = self.ktable.read().await.root().as_header();
            let id = *node.id();
            let mut table_write = self.ktable.write().await;
            if let Ok(result) = table_write.insert(node) {
                debug!("Written node in ktable: {:?}", &result);
                if let Some(pending) = result.pending_eviction() {
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
                        outbound_sender
                            .try_send((Message::Pong(my_header), vec![node_socket]))
                            .unwrap_or_else(|op| error!("Unable to send Pong {:?}", op));
                    }
                    Message::Pong(_) => {}
                    Message::FindNodes(_, _) => {
                        outbound_sender
                            .try_send((
                                Message::Nodes(
                                    my_header,
                                    NodePayload {
                                        peers: self
                                            .ktable
                                            .read()
                                            .await
                                            .closest_peers(&id)
                                            .map(|p| p.as_peer_info())
                                            .collect(),
                                    },
                                ),
                                vec![node_socket],
                            ))
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
                        debug!("Received payload with height {:?}", payload);
                        listener_sender
                            .try_send(payload.gossip_frame.clone())
                            .unwrap_or_else(|op| error!("Unable to notify client {:?}", op));
                        let table_read = self.ktable.read().await;
                        if payload.height > 0 {
                            debug!("Extracting for height {:?}", payload.height - 1);

                            table_read
                                .extract(Some((payload.height - 1).into()))
                                .for_each(|(height, nodes)| {
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
                                });
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

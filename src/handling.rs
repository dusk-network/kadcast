use std::net::SocketAddr;
use std::{convert::TryInto, sync::Arc};

use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::RwLock;
use tracing::*;

use crate::encoding::message::{BroadcastPayload, Message, NodePayload};
use crate::kbucket::Tree;
use crate::peer::{PeerInfo, PeerNode};
use crate::transport::{MessageBeanIn, MessageBeanOut};
use crate::K_K;

pub(crate) struct MessageHandler;

impl MessageHandler {
    pub(crate) fn start(
        ktable: Arc<RwLock<Tree<PeerInfo>>>,
        mut inbound_receiver: Receiver<MessageBeanIn>,
        outbound_sender: Sender<MessageBeanOut>,
        listener_sender: Sender<Vec<u8>>,
    ) {
        tokio::spawn(async move {
            debug!("MessageHandler started");
            while let Some((message, mut remote_node_addr)) =
                inbound_receiver.recv().await
            {
                debug!("Mantainer received message {:?}", message);
                remote_node_addr.set_port(message.header().sender_port);
                let remote_node = PeerNode::from_socket(remote_node_addr);
                let my_header = ktable.read().await.root().as_header();
                match ktable.write().await.insert(remote_node) {
                    Err(_) => {
                        error!("Unable to insert node");
                        continue;
                    }
                    Ok(result) => {
                        debug!("Written node in ktable: {:?}", &result);
                        if let Some(pending) = result.pending_eviction() {
                            outbound_sender
                                .try_send((
                                    Message::Ping(my_header),
                                    vec![*pending.value().address()],
                                ))
                                .unwrap_or_else(|op| {
                                    error!("Unable to send PING to pending node {:?}", op)
                                });
                        }
                    }
                }
                match message {
                    Message::Ping(_) => {
                        outbound_sender
                            .try_send((
                                Message::Pong(my_header),
                                vec![remote_node_addr],
                            ))
                            .unwrap_or_else(|op| {
                                error!("Unable to send Pong {:?}", op)
                            });
                    }
                    Message::Pong(_) => {}
                    Message::FindNodes(_, target) => {
                        outbound_sender
                            .try_send((
                                Message::Nodes(
                                    my_header,
                                    NodePayload {
                                        peers: ktable
                                            .read()
                                            .await
                                            .closest_peers::<K_K>(&target)
                                            .map(|p| p.as_peer_info())
                                            .collect(),
                                    },
                                ),
                                vec![remote_node_addr],
                            ))
                            .unwrap_or_else(|op| {
                                error!("Unable to send Nodes {:?}", op)
                            });
                    }
                    Message::Nodes(_, nodes) => {
                        if !nodes.peers.is_empty() {
                            let targets = nodes
                                .peers
                                .iter()
                                //filter out my ID to avoid loopback
                                .filter(|&n| {
                                    &n.id != my_header.binary_id.as_binary()
                                })
                                .map(|n| n.to_socket_address())
                                .collect();
                            outbound_sender
                                .try_send((Message::Ping(my_header), targets))
                                .unwrap_or_else(|op| {
                                    error!("Unable to send PING {:?}", op)
                                });
                        }
                    }
                    Message::Broadcast(_, payload) => {
                        debug!("Received payload with height {:?}", payload);
                        listener_sender
                            .try_send(payload.gossip_frame.clone())
                            .unwrap_or_else(|op| {
                                error!("Unable to notify client {:?}", op)
                            });
                        let table_read = ktable.read().await;
                        if payload.height > 0 {
                            debug!(
                                "Extracting for height {:?}",
                                payload.height - 1
                            );

                            table_read
                                .extract(Some((payload.height - 1).into()))
                                .for_each(|(height, nodes)| {
                                    let msg = Message::Broadcast(
                                        my_header,
                                        BroadcastPayload {
                                            height: height.try_into().unwrap(),
                                            gossip_frame: payload
                                                .gossip_frame
                                                .clone(), //FIX_ME: avoid clone
                                        },
                                    );
                                    let targets: Vec<SocketAddr> = nodes
                                        .map(|node| *node.value().address())
                                        .collect();
                                    outbound_sender
                                        .try_send((msg, targets))
                                        .unwrap_or_else(|op| {
                                            error!(
                                                "Unable to send broadcast {:?}",
                                                op
                                            )
                                        });
                                });
                        }
                    }
                }
            }
        });
    }
}

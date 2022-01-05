// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::net::SocketAddr;
use std::{convert::TryInto, sync::Arc};

use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::RwLock;
use tracing::*;

use crate::encoding::message::{
    BroadcastPayload, Header, Message, NodePayload,
};
use crate::kbucket::{BinaryKey, NodeInsertError, Tree};
use crate::peer::{PeerInfo, PeerNode};
use crate::transport::{MessageBeanIn, MessageBeanOut};
use crate::K_K;

/// Message metadata for incoming message notifications
#[derive(Debug)]
pub struct MessageInfo {
    pub(crate) src: SocketAddr,
    pub(crate) height: u8,
}

impl MessageInfo {
    /// Returns the incoming message sender's address
    pub fn src(&self) -> SocketAddr {
        self.src
    }
    /// Returns current kadcast broadcast height
    pub fn height(&self) -> u8 {
        self.height
    }
}

pub(crate) struct MessageHandler;

impl MessageHandler {
    pub(crate) fn start(
        ktable: Arc<RwLock<Tree<PeerInfo>>>,
        mut inbound_receiver: Receiver<MessageBeanIn>,
        outbound_sender: Sender<MessageBeanOut>,
        listener_sender: Sender<(Vec<u8>, MessageInfo)>,
        auto_propagate: bool,
        recursive_discovery: bool,
    ) {
        let nodes_reply_fn = match recursive_discovery {
            true => |header: Header, target: BinaryKey| {
                Message::FindNodes(header, target)
            },
            false => |header: Header, _: BinaryKey| Message::Ping(header),
        };
        tokio::spawn(async move {
            debug!("MessageHandler started");
            let my_header = { ktable.read().await.root().as_header() };
            while let Some((message, mut remote_node_addr)) =
                inbound_receiver.recv().await
            {
                debug!("Handler received message");
                trace!("Handler received message {:?}", message);
                remote_node_addr.set_port(message.header().sender_port);
                let remote_node = PeerNode::from_socket(remote_node_addr);

                {
                    trace!("Before access to writer");
                    let mut writer = ktable.clone().write_owned().await;
                    trace!("After access to writer");
                    match writer.insert(remote_node) {
                        Err(e) => match e {
                            NodeInsertError::Full(n) => {
                                debug!(
                                    "Unable to insert node - FULL {}",
                                    n.value().address()
                                )
                            }
                            NodeInsertError::Invalid(n) => {
                                error!(
                                    "Unable to insert node - INVALID {}",
                                    n.value().address()
                                );
                                continue;
                            }
                        },
                        Ok(result) => {
                            debug!("Written node in ktable: {:?}", &result);
                            if let Some(pending) = result.pending_eviction() {
                                outbound_sender
                                .send((
                                    Message::Ping(my_header),
                                    vec![*pending.value().address()],
                                ))
                                .await
                                .unwrap_or_else(|op| {
                                    error!("Unable to send PING to pending node {:?}", op)
                                });
                            }
                        }
                    }
                }
                match message {
                    Message::Ping(_) => {
                        outbound_sender
                            .send((
                                Message::Pong(my_header),
                                vec![remote_node_addr],
                            ))
                            .await
                            .unwrap_or_else(|op| {
                                error!("Unable to send Pong {:?}", op)
                            });
                    }
                    Message::Pong(_) => {}
                    Message::FindNodes(_, target) => {
                        outbound_sender
                            .send((
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
                            .await
                            .unwrap_or_else(|op| {
                                error!("Unable to send Nodes {:?}", op)
                            });
                    }
                    Message::Nodes(_, nodes) => {
                        if !nodes.peers.is_empty() {
                            let reader = ktable.read().await;
                            let messages = nodes
                                .peers
                                .iter()
                                //filter out my ID to avoid loopback
                                .filter(|&n| {
                                    &n.id != my_header.binary_id.as_binary()
                                })
                                .filter(|&n| {
                                    let h = my_header
                                        .binary_id
                                        .calculate_distance(&n.id);
                                    match h {
                                        None => false,
                                        Some(h) => {
                                            if reader.is_bucket_full(h) {
                                                return false;
                                            };
                                            reader.has_peer(&n.id).is_none()
                                        }
                                    }
                                })
                                .map(|n| {
                                    (
                                        //Message::FindNodes(my_header, n.id),
                                        nodes_reply_fn(my_header, n.id),
                                        vec![n.to_socket_address()],
                                    )
                                })
                                .collect::<Vec<(Message, Vec<SocketAddr>)>>();
                            for tosend in messages {
                                outbound_sender.send(tosend).await.unwrap_or_else(
                                    |op| {
                                        error!( "Unable to send FindNodes after reply {:?}", op)
                                    },
                                );
                            }
                        }
                    }
                    Message::Broadcast(_, payload) => {
                        info!("Received payload with len {:?} and height {:?} v5-rc-0", payload.gossip_frame.len(), payload.height);

                        // Aggregate message + metadata for lib client
                        let msg = payload.gossip_frame.clone();
                        let md = MessageInfo {
                            src: remote_node_addr,
                            height: payload.height,
                        };

                        // Notify lib client
                        listener_sender.send((msg, md)).await.unwrap_or_else(
                            |op| error!("Unable to notify client {:?}", op),
                        );
                        if auto_propagate && payload.height > 0 {
                            debug!(
                                "Extracting for height {:?}",
                                payload.height - 1
                            );
                            let table_read = ktable.read().await;

                            let messages: Vec<(Message, Vec<SocketAddr>)> = table_read
                                .extract(Some((payload.height - 1).into()))
                                .map(|(height, nodes)| {
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
                                    (msg, targets)
                                }).collect();
                            drop(table_read);

                            for tosend in messages {
                                outbound_sender
                                    .send(tosend)
                                    .await
                                    .unwrap_or_else(|op| {
                                        error!(
                                            "Unable to send broadcast {:?}",
                                            op
                                        )
                                    });
                            }
                        }
                    }
                }
            }
        });
    }
}

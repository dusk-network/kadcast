// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::net::SocketAddr;

use semver::{Version, VersionReq};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::*;

use crate::config::Config;
use crate::encoding::message::{
    BroadcastPayload, Header, Message, NodePayload,
};
use crate::kbucket::{BinaryKey, NodeInsertError, NodeInsertOk, Tree};
use crate::peer::{PeerInfo, PeerNode};
use crate::transport::{MessageBeanIn, MessageBeanOut};
use crate::{RwLock, K_K};

/// Message metadata for incoming message notifications
#[derive(Debug)]
pub struct MessageInfo {
    pub(crate) src: SocketAddr,
    pub(crate) height: u8,
    pub(crate) ray: Vec<u8>,
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
    /// Returns the ray-id for this message (if any)
    pub fn ray(&self) -> &[u8] {
        &self.ray
    }
}

pub(crate) struct MessageHandler {
    my_header: Header,
    ktable: RwLock<Tree<PeerInfo>>,
    outbound_sender: Sender<MessageBeanOut>,
    listener_sender: Sender<(Vec<u8>, MessageInfo)>,
    nodes_reply_fn: fn(Header, BinaryKey, Version) -> Message,
    auto_propagate: bool,
    version_req: VersionReq,
    my_version: Version,
}

impl MessageHandler {
    async fn new(
        ktable: RwLock<Tree<PeerInfo>>,
        outbound_sender: Sender<MessageBeanOut>,
        listener_sender: Sender<(Vec<u8>, MessageInfo)>,
        config: &Config,
    ) -> Self {
        let version_req = VersionReq::parse(&config.version_match)
            .expect("Invalid version req");
        let my_version =
            Version::parse(&config.version).expect("Invalid version");

        let nodes_reply_fn = match config.recursive_discovery {
            true => |header: Header, target: BinaryKey, version: Version| {
                Message::FindNodes(header, version, target)
            },
            false => |header: Header, _: BinaryKey, version: Version| {
                Message::Ping(header, version)
            },
        };
        let auto_propagate = config.auto_propagate;
        let my_header = ktable.read().await.root().to_header();

        Self {
            my_header,
            auto_propagate,
            ktable,
            listener_sender,
            outbound_sender,
            nodes_reply_fn,
            version_req,
            my_version,
        }
    }

    pub(crate) fn start(
        ktable: RwLock<Tree<PeerInfo>>,
        mut inbound_receiver: Receiver<MessageBeanIn>,
        outbound_sender: Sender<MessageBeanOut>,
        listener_sender: Sender<(Vec<u8>, MessageInfo)>,
        config: &Config,
    ) {
        let config = config.clone();
        tokio::spawn(async move {
            let handler = MessageHandler::new(
                ktable,
                outbound_sender,
                listener_sender,
                &config,
            )
            .await;
            debug!("MessageHandler started");
            while let Some((message, mut remote_peer_addr)) =
                inbound_receiver.recv().await
            {
                debug!("Handler received message");
                trace!("Handler received message {:?}", message);
                remote_peer_addr.set_port(message.header().sender_port);

                let header = message.header();
                let src = remote_peer_addr.ip();
                if !PeerNode::verify_header(header, &src) {
                    error!("Invalid Id {header:?} - from {src}");
                }

                let remote_peer = PeerNode::from_socket(
                    remote_peer_addr,
                    *message.header().binary_id(),
                    message.header().network_id,
                );

                match handler.handle_peer(remote_peer, &message).await {
                    Ok(_) => {}
                    Err(NodeInsertError::Full(n)) => {
                        debug!(
                            "Unable to insert node - FULL {}",
                            n.value().address()
                        )
                    }
                    Err(NodeInsertError::Invalid(n)) => {
                        error!(
                            "Unable to insert node - INVALID {}",
                            n.value().address()
                        );
                        continue;
                    }
                    Err(NodeInsertError::MismatchNetwork(n)) => {
                        error!(
                            "Unable to insert node - NETWORK MISMATCH {} - {}",
                            n.value().address(),
                            n.network_id,
                        );
                        continue;
                    }
                    Err(NodeInsertError::MismatchVersion(n, version)) => {
                        error!(
                            "Unable to insert node - VERSION MISMATCH {} - {version}",
                            n.value().address(),
                        );
                        continue;
                    }
                };

                handler.handle_message(message, remote_peer_addr).await;
            }
        });
    }

    async fn handle_peer(
        &self,
        remote_node: PeerNode,
        msg: &Message,
    ) -> Result<(), NodeInsertError<PeerNode>> {
        let mut table = self.ktable.write().await;

        // If it's not a BROADCAST then we should handle the version and
        // insert/update the routing table accordingly
        let result = if let Some(version) = msg.version() {
            // If version is not supported by node, discard it
            if !self.version_req.matches(version) {
                return Err(NodeInsertError::MismatchVersion(
                    remote_node,
                    version.clone(),
                ));
            }

            table.insert(remote_node)?
        } else {
            // If it's BROADCAST, and it's a new node, we should PING it in
            // order to know the version
            let peer_id = remote_node.id().as_binary();
            if table.has_peer(peer_id).is_none() {
                self.outbound_sender
                    .send((
                        Message::Ping(self.my_header, self.my_version.clone()),
                        vec![*remote_node.value().address()],
                    ))
                    .await
                    .unwrap_or_else(|e| {
                        error!("Unable to send PING to new node {e}")
                    });
                NodeInsertOk::NoAction
            } else {
                // If it's BROADCAST, and the node is already known, we just
                // refresh the routing table
                table.refresh(remote_node)?
            }
        };

        // Ping the pending node (if any)
        if let Some(pending) = result.pending_eviction() {
            self.outbound_sender
                .send((
                    Message::Ping(self.my_header, self.my_version.clone()),
                    vec![*pending.value().address()],
                ))
                .await
                .unwrap_or_else(|e| {
                    error!("Unable to send PING to pending node {e}")
                });
        };
        Ok(())
    }

    async fn handle_message(
        &self,
        message: Message,
        remote_node_addr: SocketAddr,
    ) {
        match message {
            Message::Ping(..) => self.handle_ping(remote_node_addr).await,
            Message::Pong(..) => {}
            Message::FindNodes(_, _, target) => {
                self.handle_find_nodes(remote_node_addr, &target).await
            }
            Message::Nodes(_, _, nodes) => self.handle_nodes(nodes).await,
            Message::Broadcast(_, payload) => {
                self.handle_broadcast(remote_node_addr, payload).await
            }
        }
    }

    async fn handle_ping(&self, remote_node_addr: SocketAddr) {
        self.outbound_sender
            .send((
                Message::Pong(self.my_header, self.my_version.clone()),
                vec![remote_node_addr],
            ))
            .await
            .unwrap_or_else(|e| error!("Unable to send Pong {e}"));
    }

    async fn handle_find_nodes(
        &self,
        remote_node_addr: SocketAddr,
        target: &BinaryKey,
    ) {
        let peers = self
            .ktable
            .read()
            .await
            .closest_peers::<K_K>(target)
            .map(|p| p.as_peer_info())
            .collect();
        let message = Message::Nodes(
            self.my_header,
            self.my_version.clone(),
            NodePayload { peers },
        );
        self.outbound_sender
            .send((message, vec![remote_node_addr]))
            .await
            .unwrap_or_else(|e| error!("Unable to send Nodes {e}"));
    }

    async fn handle_nodes(&self, nodes: NodePayload) {
        let peers = nodes.peers;
        if peers.is_empty() {
            return;
        }
        let reader = self.ktable.read().await;
        let messages: Vec<_> = peers
            .iter()
            //filter out my ID to avoid loopback
            .filter(|&n| &n.id != self.my_header.binary_id().as_binary())
            .filter(|&n| {
                let h = self.my_header.binary_id().calculate_distance(&n.id);
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
                    (self.nodes_reply_fn)(
                        self.my_header,
                        n.id,
                        self.my_version.clone(),
                    ),
                    vec![n.to_socket_address()],
                )
            })
            .collect();
        for tosend in messages {
            self.outbound_sender.send(tosend).await.unwrap_or_else(|e| {
                error!("Unable to send FindNodes after reply {e}")
            });
        }
    }

    async fn handle_broadcast(
        &self,
        src: SocketAddr,
        payload: BroadcastPayload,
    ) {
        let height = payload.height;
        let gossip_frame = payload.gossip_frame;
        debug!(
            "Received payload with height {height} and len {}",
            gossip_frame.len()
        );
        let ray = payload.ray;

        // Aggregate message + metadata for lib client
        let msg = gossip_frame.clone();
        let md = MessageInfo { src, height, ray };

        // Notify lib client
        self.listener_sender
            .send((msg, md))
            .await
            .unwrap_or_else(|e| error!("Unable to notify client {e}"));

        if self.auto_propagate && height > 0 {
            let new_height = height - 1;
            debug!("Extracting for height {new_height}");

            let messages: Vec<_> = {
                let table_read = self.ktable.read().await;
                let target_nodes = table_read.extract(Some(new_height));

                target_nodes
                    .map(|(height, nodes)| {
                        //FIX_ME: avoid clone
                        let gossip_frame = gossip_frame.clone();
                        let payload = BroadcastPayload {
                            height,
                            gossip_frame,
                            ray: vec![], /* ray will be set while sending
                                          * according to the encoder */
                        };
                        let msg = Message::Broadcast(self.my_header, payload);
                        let targets =
                            nodes.map(|node| *node.value().address()).collect();
                        (msg, targets)
                    })
                    .collect()
            };

            for msg in messages {
                self.outbound_sender
                    .send(msg)
                    .await
                    .unwrap_or_else(|e| error!("Unable to send broadcast {e}"));
            }
        }
    }
}

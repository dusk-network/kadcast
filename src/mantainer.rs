// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;

use tokio::sync::mpsc::Sender;
use tracing::{error, info};

use crate::encoding::message::{Header, Message};
use crate::kbucket::Tree;
use crate::peer::PeerInfo;
use crate::transport::MessageBeanOut;
use crate::RwLock;

pub(crate) struct TableMantainer {
    bootstrapping_nodes: Vec<String>,
    ktable: RwLock<Tree<PeerInfo>>,
    outbound_sender: Sender<MessageBeanOut>,
    my_ip: SocketAddr,
    header: Header,
}

impl TableMantainer {
    pub(crate) fn start(
        bootstrapping_nodes: Vec<String>,
        ktable: RwLock<Tree<PeerInfo>>,
        outbound_sender: Sender<MessageBeanOut>,
    ) {
        tokio::spawn(async move {
            let my_ip = *ktable.read().await.root().value().address();
            let header = ktable.read().await.root().as_header();

            let mantainer = Self {
                bootstrapping_nodes,
                ktable,
                outbound_sender,
                my_ip,
                header,
            };
            mantainer.monitor_buckets().await;
        });
    }

    /// Check if the peer need to contact the bootstrappers in order to join the
    /// network
    async fn need_bootstrappers(&self) -> bool {
        let binary_key = self.header.binary_id.as_binary();
        self.ktable
            .read()
            .await
            .closest_peers::<10>(binary_key)
            .count()
            < 3
    }

    /// Return a vector containing the Socket Addresses bound to the provided
    /// nodes
    fn bootstrapping_nodes_addr(&self) -> Vec<SocketAddr> {
        self.bootstrapping_nodes
            .iter()
            .flat_map(|boot| {
                boot.to_socket_addrs().unwrap_or_else(|e| {
                    error!("Unable to resolve domain for {} - {}", boot, e);
                    vec![].into_iter()
                })
            })
            .filter(|socket| socket != &self.my_ip)
            .collect()
    }

    /// Try to contact the bootstrappers node until no needed anymore
    async fn contact_bootstrappers(&self) {
        while self.need_bootstrappers().await {
            info!("TableMantainer::contact_bootstrappers");
            let bootstrapping_nodes_addr = self.bootstrapping_nodes_addr();
            let binary_key = self.header.binary_id.as_binary();
            let find_nodes = Message::FindNodes(self.header, *binary_key);
            self.send((find_nodes, bootstrapping_nodes_addr)).await;
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }

    async fn send(&self, message: MessageBeanOut) {
        self.outbound_sender
            .send(message)
            .await
            .unwrap_or_else(|e| {
                error!("Unable to send message from mantainer {e}")
            })
    }

    /// This is the main function of this utility class. It's responsible to:
    /// 1. Contact bootstrappers (if needed)
    /// 2. Ping idle buckets
    /// 3. Remove idles nodes from buckets
    async fn monitor_buckets(&self) {
        info!("TableMantainer::monitor_buckets started");
        let idle_time: Duration =
            { self.ktable.read().await.config.bucket_ttl };
        loop {
            self.contact_bootstrappers().await;
            info!("TableMantainer::monitor_buckets back to sleep");

            tokio::time::sleep(idle_time).await;

            info!("TableMantainer::monitor_buckets woke up");
            self.ping_idle_buckets().await;

            info!("TableMantainer::monitor_buckets removing idle nodes");
            self.ktable.write().await.remove_idle_nodes();
        }
    }

    /// Search for idle buckets (no message received) and try to contact some of
    /// the belonging nodes
    async fn ping_idle_buckets(&self) {
        let table_lock_read = self.ktable.read().await;

        let find_node_messages = table_lock_read
            .idle_buckets()
            .flat_map(|(_, idle_nodes)| idle_nodes)
            .map(|target| {
                (
                    Message::FindNodes(self.header, *target.id().as_binary()),
                    //TODO: Extract alpha nodes
                    vec![*target.value().address()],
                )
            });
        for find_node in find_node_messages {
            self.send(find_node).await;
        }
    }
}

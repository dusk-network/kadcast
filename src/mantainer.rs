// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;

use tokio::sync::mpsc::Sender;
use tracing::*;

use crate::encoding::message::Message;
use crate::kbucket::Tree;
use crate::peer::PeerInfo;
use crate::transport::MessageBeanOut;
use crate::RwLock;

pub(crate) struct TableMantainer {}

impl TableMantainer {
    pub async fn start(
        bootstrapping_nodes: Vec<String>,
        ktable: RwLock<Tree<PeerInfo>>,
        outbound_sender: Sender<MessageBeanOut>,
    ) {
        let my_ip = *ktable.read().await.root().value().address();
        let header = ktable.read().await.root().as_header();
        let binary_key = header.binary_id.as_binary();

        while ktable.read().await.closest_peers::<10>(binary_key).count() < 3 {
            let bootstrapping_nodes_addr: Vec<SocketAddr> = bootstrapping_nodes
                .iter()
                .flat_map(|boot| {
                    boot.to_socket_addrs().unwrap_or_else(|e| {
                        error!("Unable to resolve domain for {} - {}", boot, e);
                        vec![].into_iter()
                    })
                })
                .filter(|socket| socket != &my_ip)
                .collect();
            let find_nodes = Message::FindNodes(header, *binary_key);
            outbound_sender
                .send((find_nodes, bootstrapping_nodes_addr.clone()))
                .await
                .unwrap_or_else(|op| error!("Unable to send generic {:?}", op));
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
        TableMantainer::monitor_buckets(ktable.clone(), outbound_sender).await;
    }

    async fn monitor_buckets(
        ktable: RwLock<Tree<PeerInfo>>,
        outbound_sender: Sender<MessageBeanOut>,
    ) {
        info!("TableMantainer::monitor_buckets started");
        let idle_time: Duration = { ktable.read().await.config.bucket_ttl };
        loop {
            tokio::time::sleep(idle_time).await;
            info!("TableMantainer::monitor_buckets woke up");
            TableMantainer::ping_idle_buckets(&ktable, &outbound_sender).await;
            ktable.write().await.remove_idle_nodes();
            info!("TableMantainer::monitor_buckets back to sleep");
        }
    }

    async fn ping_idle_buckets(
        ktable: &RwLock<Tree<PeerInfo>>,
        outbound_sender: &Sender<MessageBeanOut>,
    ) {
        let table_lock_read = ktable.read().await;
        let root_header = table_lock_read.root().as_header();

        let idles = table_lock_read
            .idle_buckets()
            .flat_map(|(_, target)| target)
            .map(|target| {
                (
                    Message::FindNodes(root_header, *target.id().as_binary()),
                    //TODO: Extract alpha nodes
                    vec![*target.value().address()],
                )
            });
        for idle in idles {
            outbound_sender.send(idle).await.unwrap_or_else(|op| {
                error!("Unable to send broadcast {:?}", op)
            });
        }
    }
}

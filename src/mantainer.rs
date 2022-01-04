// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;
use tracing::*;

use crate::encoding::message::Message;
use crate::kbucket::Tree;
use crate::peer::PeerInfo;
use crate::transport::MessageBeanOut;

pub(crate) struct TableMantainer {}

impl TableMantainer {
    pub async fn start(
        bootstrapping_nodes: Vec<String>,
        ktable: Arc<RwLock<Tree<PeerInfo>>>,
        outbound_sender: Sender<MessageBeanOut>,
    ) {
        let find_nodes = Message::FindNodes(
            ktable.read().await.root().as_header(),
            *ktable.read().await.root().id().as_binary(),
        );
        let bootstrapping_nodes_addr = bootstrapping_nodes
            .iter()
            .flat_map(|boot| {
                boot.to_socket_addrs()
                    .expect("Unable to resolve domain for {}")
            })
            .collect();
        outbound_sender
            .send((find_nodes, bootstrapping_nodes_addr))
            .await
            .unwrap_or_else(|op| error!("Unable to send generic {:?}", op));
        TableMantainer::monitor_buckets(ktable.clone(), outbound_sender).await;
    }

    async fn monitor_buckets(
        ktable: Arc<RwLock<Tree<PeerInfo>>>,
        outbound_sender: Sender<MessageBeanOut>,
    ) {
        debug!("TableMantainer::monitor_buckets started");
        let idle_time: Duration = ktable.read().await.config.bucket_ttl;
        loop {
            tokio::time::sleep(idle_time).await;
            debug!("TableMantainer::monitor_buckets woke up");
            let table_lock_read = ktable.read().await;
            let root = table_lock_read.root();

            let idles = table_lock_read
                .idle_buckets()
                .flat_map(|(_, target)| target)
                .map(|target| {
                    (
                        Message::FindNodes(
                            root.as_header(),
                            *target.id().as_binary(),
                        ),
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
}

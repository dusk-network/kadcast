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
        ktable: &Arc<RwLock<Tree<PeerInfo>>>,
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
            table_lock_read.idle_buckets().flat_map(|h| h.1).for_each(
                |target| {
                    outbound_sender
                        .try_send((
                            Message::FindNodes(
                                root.as_header(),
                                *target.id().as_binary(),
                            ),
                            vec![*target.value().address()],
                        ))
                        .unwrap_or_else(|op| {
                            error!("Unable to send broadcast {:?}", op)
                        });
                },
            );
        }
    }
}

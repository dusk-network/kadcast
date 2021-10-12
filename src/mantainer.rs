use std::{net::ToSocketAddrs, sync::Arc};

use tokio::sync::{
    mpsc::{Receiver, Sender},
    RwLock,
};

use crate::{
    encoding::{message::Message, payload::NodePayload},
    kbucket::Tree,
    peer::{PeerInfo, PeerNode},
    transport::{MessageBeanIn, MessageBeanOut},
};

pub(crate) struct TableMantainer {
    bootstrapping_nodes: Vec<String>,
    ktable: Arc<RwLock<Tree<PeerInfo>>>,
}

impl TableMantainer {
    pub async fn start(
        self,
        mut inbound_receiver: Receiver<MessageBeanIn>,
        outbound_sender: Sender<MessageBeanOut>,
    ) {
        let find_node = Message::FindNodes(
            self.ktable.read().await.root().as_header(),
            NodePayload {
                peers: vec![self.ktable.read().await.root().as_peer_info()],
            },
        );
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
            .unwrap_or_else(|op| println!("Unable to send generic {:?}", op));
        tokio::spawn(async move {
            while let Some((message, mut node_socket)) = inbound_receiver.recv().await {
                println!("Mantainer received message {:?}", message);
                node_socket.set_port(message.header().sender_port);
                let node = PeerNode::from_socket(node_socket);
                let my_header = self.ktable.read().await.root().as_header();
                if let Ok(node) = self.ktable.write().await.insert(node) {
                    println!("Written node in ktable: {:?}", &node);
                    match message {
                        Message::Ping(_) => {
                            let pong = Message::Pong(my_header);
                            outbound_sender
                                .try_send((pong, vec![node_socket]))
                                // .await
                                .unwrap_or_else(|op| println!("Unable to send ping {:?}", op));
                        }
                        Message::Pong(_) => {}
                        Message::FindNodes(_, _) => todo!(),
                        Message::Nodes(_, nodes) => {
                            let targets =
                                nodes.peers.iter().map(|n| n.to_socket_address()).collect();
                            outbound_sender
                                .try_send((Message::Ping(my_header), targets))
                                // .await
                                .unwrap_or_else(|op| println!("Unable to send nodes {:?}", op));
                        }
                        Message::Broadcast(_, _) => todo!(),
                    };
                } else {
                    println!("Unable to insert node");
                }
            }
        });
    }

    pub fn new(bootstrapping_nodes: Vec<String>, ktable: Arc<RwLock<Tree<PeerInfo>>>) -> Self {
        TableMantainer {
            bootstrapping_nodes,
            ktable,
        }
    }
}

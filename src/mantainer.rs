use std::net::ToSocketAddrs;

use tokio::sync::mpsc::{Receiver, Sender};

use crate::{
    encoding::{message::Message, payload::NodePayload},
    kbucket::Tree,
    peer::{PeerInfo, PeerNode},
    transport::{MessageBeanIn, MessageBeanOut},
};

pub(crate) struct TableMantainer {
    bootstrapping_nodes: Vec<String>,
    ktable: Tree<PeerInfo>,
    outbound_sender: Sender<MessageBeanOut>,
    inbound_receiver: Receiver<MessageBeanIn>,
}

impl TableMantainer {
    pub async fn start(mut self) {
        let find_node = Message::FindNodes(
            self.ktable().root().as_header(),
            NodePayload {
                peers: vec![self.ktable().root().as_peer_info()],
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
        self.outbound_sender
            .send((find_node, bootstrapping_nodes))
            .await
            .unwrap_or_else(|op| println!("Unable to send {:?}", op));
        // tokio::spawn(async move {
        while let Some((message, mut node_socket)) = self.inbound_receiver.recv().await {
            println!("received message in mantainer {:?}", message);
            node_socket.set_port(message.header().sender_port);
            let node = PeerNode::from_socket(node_socket);
            if let Ok(node) = self.ktable.insert(node) {
                println!("received node in mantainer {:?}", &node);
                let my_header = self.ktable().root().as_header();
                match message {
                    Message::Ping(_) => {
                        let pong = Message::Pong(my_header);
                        self.outbound_sender
                            .send((pong, vec![node_socket]))
                            .await
                            .unwrap_or_else(|op| println!("Unable to send {:?}", op));
                    }
                    Message::Pong(_) => {}
                    Message::FindNodes(_, _) => todo!(),
                    Message::Nodes(_, nodes) => {
                        let targets = nodes.peers.iter().map(|n| n.to_socket_address()).collect();
                        self.outbound_sender
                            .send((Message::Ping(my_header), targets))
                            .await
                            .unwrap_or_else(|op| println!("Unable to send {:?}", op));
                    }
                    Message::Broadcast(_, _) => todo!(),
                };
            } else {
                println!("Unable to insert node");
            }
        }
        // });
    }

    pub fn new(
        bootstrapping_nodes: Vec<String>,
        ktable: Tree<PeerInfo>,
        outbound_sender: Sender<MessageBeanOut>,
        inbound_receiver: Receiver<MessageBeanIn>,
    ) -> Self {
        TableMantainer {
            bootstrapping_nodes,
            ktable,
            outbound_sender,
            inbound_receiver,
        }
    }

    pub fn ktable(&self) -> &Tree<PeerInfo> {
        &self.ktable
    }
}

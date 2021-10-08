use std::net::ToSocketAddrs;

use tokio::sync::mpsc::{Receiver, Sender};

use crate::{
    encoding::{message::KadcastMessage, payload::NodePayload},
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
        let find_node = KadcastMessage::FindNodes(
            self.ktable().root().as_header(),
            NodePayload {
                peers: vec![self.ktable().root().as_peer_info()],
            },
        );
        let bootstrapping_nodes: Vec<_> = self
            .bootstrapping_nodes
            .iter()
            .flat_map(|boot| boot.to_socket_addrs().expect("Unable to resolve domain"))
            .collect();
        let _ = self
            .outbound_sender
            .send((find_node, bootstrapping_nodes))
            .await;

        tokio::spawn(async move {
            while let Some((message, from)) = self.inbound_receiver.recv().await {
                println!("received message in mantainer {:?}", message);
                let node = PeerNode::from_socket(from);
                println!("received node in mantainer {:?}", node);
                match message {
                    KadcastMessage::Ping(_header) => {
                        let res = self.ktable.insert(node);
                        println!("res {:?}", res);
                    }
                    _ => {
                        println!("not handled yet")
                    }
                    // KadcastMessage::Pong(_) => todo!(),
                    // KadcastMessage::FindNodes(_, _) => todo!(),
                    // KadcastMessage::Nodes(_, _) => todo!(),
                    // KadcastMessage::Broadcast(_, _) => todo!(),
                };
                // outbound_sender.send((KadcastMessage::Pong()))
                // message.0.marshal_binary(writer)
                //Send message to destinations
                // todo!()
            }
        });
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

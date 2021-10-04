use std::net::SocketAddr;

use crate::encoding::message::KadcastMessage;

pub(crate) struct WireNetwork {
    public_address: SocketAddr,
}

impl WireNetwork {
    pub fn new(public_ip: &str) -> Self {
        WireNetwork {
            public_address: public_ip
                .parse()
                .expect("Unable to parse public_ip address"),
        }
    }

    pub fn start(&self) {
        // tokio::spawn(async move {
        //     loop {
        //         let mut buffer: Vec<u8>  = Vec::new();
        //         let (_, src) = server_rx.recv_from(&mut buffer).await.unwrap();
        //         let packet = Packet {
        //             buf: buffer,
        //             addr: src,
        //         };

        //         // Spawn heavy work as task, to continue reading from
        //         // socket while processing packets.
        //         let mut shared_tx = tx.clone();
        //         tokio::spawn(async move {
        //             //
        //             // do some heavy work with the packet
        //             //
        //             shared_tx.send(packet).await.unwrap(); // packet work is done send it back to the client
        //         });
        //     }
        // });
    }

    pub fn send_message(&self, message: &KadcastMessage, target: &SocketAddr) {
        todo!()
    }
}

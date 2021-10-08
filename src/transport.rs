use std::{
    io::{BufReader, Cursor},
    net::SocketAddr,
};

use tokio::{
    net::UdpSocket,
    sync::mpsc::{self, Receiver, Sender},
};

use crate::encoding::{message::KadcastMessage, Marshallable};

pub(crate) type MessageBeanOut = (KadcastMessage, Vec<SocketAddr>);
pub(crate) type MessageBeanIn = (KadcastMessage, SocketAddr);

pub(crate) struct WireNetwork {
    public_address: SocketAddr,
    outbound_channel_tx: Sender<MessageBeanOut>,
    outbound_channel_rx: Receiver<MessageBeanOut>,
    inbound_channel_tx: Sender<MessageBeanIn>,
}

impl WireNetwork {
    pub async fn start(mut self) {
        let a = WireNetwork::listen_out(&mut self.outbound_channel_rx);
        let b = WireNetwork::listen_in(self.public_address, self.inbound_channel_tx.clone());
        tokio::join!(a, b);
    }

    async fn listen_in(
        public_address: SocketAddr,
        // outbound_handle: JoinHandle<()>,
        inbound_channel_tx: Sender<MessageBeanIn>,
    ) {
        let socket = UdpSocket::bind(public_address).await.unwrap();
        println!("Listening on: {}", socket.local_addr().unwrap());
        loop {
            //instantiate udp server
            //foreach message received do:
            let mut bytes = vec![0; 65_535];
            let received = socket.recv_from(&mut bytes).await;
            // .map(|received| {
            // let bytes = vec![0u8, 0u8];
            let c = Cursor::new(&bytes);
            let mut reader = BufReader::new(c);
            match KadcastMessage::unmarshal_binary(&mut reader) {
                Ok(deser) => {
                    println!("Received {:?}", deser);
                    let _ = inbound_channel_tx.send((deser, received.unwrap().1)).await;
                    //FIX_ME
                }
                Err(e) => println!("Error deser from {} - {}", received.unwrap().1, e),
            }
        }
    }

    async fn listen_out(outbound_channel_rx: &mut Receiver<MessageBeanOut>) {
        // tokio::spawn(async move {
        while let Some((message, to)) = outbound_channel_rx.recv().await {
            println!("received message to send to ({:?}) {:?} ", to, message);
        }
        // });
    }

    pub fn new(public_ip: &str, inbound_channel_tx: Sender<MessageBeanIn>) -> Self {
        let (outbound_channel_tx, outbound_channel_rx) = mpsc::channel(32);
        WireNetwork {
            public_address: public_ip
                .parse()
                .expect("Unable to parse public_ip address"),
            outbound_channel_tx,
            outbound_channel_rx,
            inbound_channel_tx,
        }
    }

    pub fn sender(&self) -> Sender<MessageBeanOut> {
        self.outbound_channel_tx.clone()
    }
}

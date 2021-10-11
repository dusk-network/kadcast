use std::net::SocketAddr;

use tokio::{
    io,
    net::UdpSocket,
    sync::mpsc::{self, Receiver, Sender},
};

use crate::encoding::{message::Message, Marshallable};

pub(crate) type MessageBeanOut = (Message, Vec<SocketAddr>);
pub(crate) type MessageBeanIn = (Message, SocketAddr);

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
        let _ = tokio::join!(a, b);
    }

    async fn listen_in(
        public_address: SocketAddr,
        inbound_channel_tx: Sender<MessageBeanIn>,
    ) -> io::Result<()> {
        let socket = UdpSocket::bind(public_address).await?;
        println!("Listening on: {}", socket.local_addr()?);
        loop {
            //instantiate udp server
            //foreach message received do:
            let mut bytes = [0; 65_535];
            let received = socket.recv_from(&mut bytes).await?;
            match Message::unmarshal_binary(&mut &bytes[..]) {
                Ok(deser) => {
                    println!("Received {:?}", deser);
                    let _ = inbound_channel_tx.send((deser, received.1)).await;
                }
                Err(e) => println!("Error deser from {} - {}", received.1, e),
            }
        }
    }

    async fn listen_out(outbound_channel_rx: &mut Receiver<MessageBeanOut>) -> io::Result<()> {
        loop {
            if let Some((message, to)) = outbound_channel_rx.recv().await {
                println!("received message to send to ({:?}) {:?} ", to, message);
            }
        }
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

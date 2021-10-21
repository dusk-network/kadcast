use std::{error::Error, net::SocketAddr};

use tokio::{
    io,
    net::UdpSocket,
    sync::mpsc::{Receiver, Sender},
};
use tracing::*;

use crate::{
    encoding::{message::Message, Marshallable},
    MAX_DATAGRAM_SIZE,
};

pub(crate) type MessageBeanOut = (Message, Vec<SocketAddr>);
pub(crate) type MessageBeanIn = (Message, SocketAddr);

pub(crate) struct WireNetwork {}

impl WireNetwork {
    pub async fn start(
        inbound_channel_tx: &Sender<MessageBeanIn>,
        public_ip: &str,
        outbound_channel_rx: Receiver<MessageBeanOut>,
    ) {
        // let (outbound_channel_tx, outbound_channel_rx) = mpsc::channel(32);
        let public_address = public_ip
            .parse()
            .expect("Unable to parse public_ip address");
        let a = WireNetwork::listen_out(outbound_channel_rx);
        let b = WireNetwork::listen_in(public_address, inbound_channel_tx.clone());
        let _ = tokio::join!(a, b);
    }

    async fn listen_in(
        public_address: SocketAddr,
        inbound_channel_tx: Sender<MessageBeanIn>,
    ) -> io::Result<()> {
        let socket = UdpSocket::bind(public_address).await?;
        info!("Listening on: {}", socket.local_addr()?);
        loop {
            let mut bytes = [0; MAX_DATAGRAM_SIZE];
            let received = socket.recv_from(&mut bytes).await?;
            match Message::unmarshal_binary(&mut &bytes[..]) {
                Ok(deser) => {
                    debug!("> Received {:?}", deser);
                    let _ = inbound_channel_tx.try_send((deser, received.1));
                }
                Err(e) => error!("Error deser from {} - {}", received.1, e),
            }
        }
    }

    async fn listen_out(mut outbound_channel_rx: Receiver<MessageBeanOut>) -> io::Result<()> {
        loop {
            if let Some((message, to)) = outbound_channel_rx.recv().await {
                debug!("< Message to send to ({:?}) - {:?} ", to, message);
                let mut bytes = vec![];
                message.marshal_binary(&mut bytes).unwrap();
                for remote_addr in to.iter() {
                    WireNetwork::send(&bytes, remote_addr).await.unwrap();
                }
            }
        }
    }

    async fn send(data: &[u8], remote_addr: &SocketAddr) -> Result<(), Box<dyn Error>> {
        let local_addr: SocketAddr = if remote_addr.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        }
        .parse()?;
        let socket = UdpSocket::bind(local_addr).await?;
        // const MAX_DATAGRAM_SIZE: usize = 65_507;
        socket.connect(&remote_addr).await?;
        socket.send(data).await?;
        Ok(())
    }
}

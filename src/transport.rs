// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::{error::Error, net::SocketAddr};

use tokio::{
    io,
    net::UdpSocket,
    sync::mpsc::{Receiver, Sender},
};
use tracing::*;

use crate::{
    encoding::{message::Message, Marshallable},
    peer::PeerNode,
    transport::encoding::{Encoder, RaptorQEncoder},
    MAX_DATAGRAM_SIZE,
};

pub(crate) type MessageBeanOut = (Message, Vec<SocketAddr>);
pub(crate) type MessageBeanIn = (Message, SocketAddr);

pub(crate) struct WireNetwork {}

mod encoding;

impl WireNetwork {
    pub async fn start(
        inbound_channel_tx: Sender<MessageBeanIn>,
        listen_address: String,
        outbound_channel_rx: Receiver<MessageBeanOut>,
    ) {
        let listen_address = listen_address
            .parse()
            .expect("Unable to parse public_ip address");
        let a = WireNetwork::listen_out(outbound_channel_rx);
        let b =
            WireNetwork::listen_in(listen_address, inbound_channel_tx.clone());
        let _ = tokio::join!(a, b);
    }

    async fn listen_in(
        listen_address: SocketAddr,
        inbound_channel_tx: Sender<MessageBeanIn>,
    ) -> io::Result<()> {
        debug!("WireNetwork::listen_in started");
        let mut decoder = RaptorQEncoder::new();
        let socket = UdpSocket::bind(listen_address)
            .await
            .expect("Unable to bind address");
        info!("Listening on: {}", socket.local_addr()?);
        loop {
            let mut bytes = [0; MAX_DATAGRAM_SIZE];
            let (_, remote_address) = socket.recv_from(&mut bytes).await?;

            match Message::unmarshal_binary(&mut &bytes[..]) {
                Ok(deser) => {
                    trace!("> Received {:?}", deser);
                    let to_process = decoder.decode(deser);
                    if let Some(message) = to_process {
                        let valid_header = PeerNode::verify_header(
                            message.header(),
                            &remote_address.ip(),
                        );
                        match valid_header {
                            true => {
                                let _ = inbound_channel_tx
                                    .send((message, remote_address))
                                    .await
                                    .unwrap_or_else(
                                        |op| error!("Unable to send to inbound channel {:?}", op),
                                    );
                            }
                            false => {
                                error!(
                                    "Invalid Id {:?} - {}",
                                    message.header(),
                                    &remote_address.ip()
                                );
                            }
                        }
                    }
                }
                Err(e) => error!("Error deser from {} - {}", remote_address, e),
            }
        }
    }

    async fn listen_out(
        mut outbound_channel_rx: Receiver<MessageBeanOut>,
    ) -> io::Result<()> {
        debug!("WireNetwork::listen_out started");
        let output_sockets = MultipleSocket::new().await?;
        loop {
            if let Some((message, to)) = outbound_channel_rx.recv().await {
                trace!("< Message to send to ({:?}) - {:?} ", to, message);
                for chunk in RaptorQEncoder::encode(message).iter() {
                    let bytes = chunk.bytes();
                    for remote_addr in to.iter() {
                        let _ = output_sockets
                            .send(&bytes, remote_addr)
                            .await
                            .map_err(|e| warn!("Unable to send msg {}", e));
                    }
                }
            }
        }
    }
}

struct MultipleSocket {
    ipv4: UdpSocket,
    ipv6: UdpSocket,
}

impl MultipleSocket {
    async fn new() -> io::Result<Self> {
        let ipv4 = UdpSocket::bind("0.0.0.0:0").await?;
        let ipv6 = UdpSocket::bind("[::]:0").await?;
        Ok(MultipleSocket { ipv4, ipv6 })
    }
    async fn send(
        &self,
        data: &[u8],
        remote_addr: &SocketAddr,
    ) -> Result<(), Box<dyn Error>> {
        match remote_addr.is_ipv4() {
            true => self.ipv4.send_to(data, &remote_addr).await?,
            false => self.ipv6.send_to(data, &remote_addr).await?,
        };
        Ok(())
    }
}

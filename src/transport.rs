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
    transport::encoding::Encoder,
    MAX_DATAGRAM_SIZE,
};

pub(crate) type MessageBeanOut = (Message, Vec<SocketAddr>);
pub(crate) type MessageBeanIn = (Message, SocketAddr);

pub(crate) struct WireNetwork {}

mod encoding;
use encoding::PlainEncoder;

impl WireNetwork {
    pub async fn start(
        inbound_channel_tx: Sender<MessageBeanIn>,
        public_ip: String,
        outbound_channel_rx: Receiver<MessageBeanOut>,
    ) {
        let public_address = public_ip
            .parse()
            .expect("Unable to parse public_ip address");
        let a = WireNetwork::listen_out(outbound_channel_rx);
        let b =
            WireNetwork::listen_in(public_address, inbound_channel_tx.clone());
        let _ = tokio::join!(a, b);
    }

    async fn listen_in(
        public_address: SocketAddr,
        inbound_channel_tx: Sender<MessageBeanIn>,
    ) -> io::Result<()> {
        debug!("WireNetwork::listen_in started");
        let encoder = PlainEncoder {};
        let socket = UdpSocket::bind(public_address)
            .await
            .expect("Unable to bind address");
        info!("Listening on: {}", socket.local_addr()?);
        loop {
            let mut bytes = [0; MAX_DATAGRAM_SIZE];
            let received = socket.recv_from(&mut bytes).await?;

            match Message::unmarshal_binary(&mut &bytes[..]) {
                Ok(deser) => {
                    trace!("> Received {:?}", deser);
                    let to_process = {
                        if let Message::Broadcast(header, payload) = deser {
                            encoder.decode(payload).map(|decoded_payload| {
                                Message::Broadcast(header, decoded_payload)
                            })
                        } else {
                            Some(deser)
                        }
                    };
                    if let Some(message) = to_process {
                        let _ =
                            inbound_channel_tx.try_send((message, received.1));
                    }
                }
                Err(e) => error!("Error deser from {} - {}", received.1, e),
            }
        }
    }

    async fn listen_out(
        mut outbound_channel_rx: Receiver<MessageBeanOut>,
    ) -> io::Result<()> {
        debug!("WireNetwork::listen_out started");
        loop {
            if let Some((message, to)) = outbound_channel_rx.recv().await {
                trace!("< Message to send to ({:?}) - {:?} ", to, message);
                let mut bytes = vec![];
                message.marshal_binary(&mut bytes).unwrap();
                for remote_addr in to.iter() {
                    WireNetwork::send(&bytes, remote_addr).await.unwrap();
                }
            }
        }
    }

    async fn send(
        data: &[u8],
        remote_addr: &SocketAddr,
    ) -> Result<(), Box<dyn Error>> {
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

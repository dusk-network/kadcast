// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::{collections::HashMap, error::Error, net::SocketAddr};

use tokio::{
    io,
    net::UdpSocket,
    sync::mpsc::{Receiver, Sender},
};
use tracing::*;

use crate::{
    encoding::{message::Message, Marshallable},
    peer::PeerNode,
    transport::encoding::{
        Configurable, Decoder, Encoder, TransportDecoder, TransportEncoder,
    },
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
        conf: HashMap<String, String>,
    ) {
        let listen_address = listen_address
            .parse()
            .expect("Unable to parse public_ip address");
        let a = WireNetwork::listen_out(outbound_channel_rx, &conf);
        let b = WireNetwork::listen_in(
            listen_address,
            inbound_channel_tx.clone(),
            &conf,
        );
        let _ = tokio::join!(a, b);
    }

    async fn listen_in(
        listen_address: SocketAddr,
        inbound_channel_tx: Sender<MessageBeanIn>,
        conf: &HashMap<String, String>,
    ) -> io::Result<()> {
        debug!("WireNetwork::listen_in started");
        let mut decoder = TransportDecoder::configure(conf);
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
                                //FIX_ME: use send.await instead of try_send
                                let _ = inbound_channel_tx
                                    .try_send((message, remote_address));
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
        conf: &HashMap<String, String>,
    ) -> io::Result<()> {
        debug!("WireNetwork::listen_out started");
        let encoder = TransportEncoder::configure(conf);
        loop {
            if let Some((message, to)) = outbound_channel_rx.recv().await {
                trace!("< Message to send to ({:?}) - {:?} ", to, message);
                for chunk in encoder.encode(message).iter() {
                    let bytes = chunk.bytes();
                    for remote_addr in to.iter() {
                        let _ = WireNetwork::send(&bytes, remote_addr)
                            .await
                            .map_err(|e| warn!("Unable to send msg {}", e));
                    }
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

pub fn default_configuration() -> HashMap<String, String> {
    let mut conf = TransportEncoder::default_configuration();
    conf.extend(TransportDecoder::default_configuration());
    conf
}

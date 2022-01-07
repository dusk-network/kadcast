// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::{collections::HashMap, net::SocketAddr, time::Duration};

use socket2::SockRef;
use tokio::{
    io,
    net::UdpSocket,
    sync::mpsc::{Receiver, Sender},
    time::{self},
};
use tracing::*;

use crate::{
    encoding::{message::Message, Marshallable},
    peer::PeerNode,
    transport::{
        encoding::{
            Configurable, Decoder, Encoder, TransportDecoder, TransportEncoder,
        },
        sockets::MultipleOutSocket,
    },
};
pub(crate) type MessageBeanOut = (Message, Vec<SocketAddr>);
pub(crate) type MessageBeanIn = (Message, SocketAddr);

const MAX_DATAGRAM_SIZE: usize = 65_507;
pub(crate) struct WireNetwork {}

mod encoding;
mod sockets;

impl WireNetwork {
    pub fn start(
        inbound_channel_tx: Sender<MessageBeanIn>,
        listen_address: String,
        outbound_channel_rx: Receiver<MessageBeanOut>,
        conf: HashMap<String, String>,
    ) {
        let listen_address = listen_address
            .parse()
            .expect("Unable to parse public_ip address");
        let c = conf.clone();
        tokio::spawn(async move {
            WireNetwork::listen_out(outbound_channel_rx, &c)
                .await
                .unwrap_or_else(|op| error!("Error in listen_out {:?}", op));
        });
        tokio::spawn(async move {
            WireNetwork::listen_in(
                listen_address,
                inbound_channel_tx.clone(),
                &conf,
            )
            .await
            .unwrap_or_else(|op| error!("Error in listen_in {:?}", op));
        });
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
        WireNetwork::configure_socket(&socket, conf)?;
        info!("Listening on: {}", socket.local_addr()?);
        loop {
            let mut bytes = [0; MAX_DATAGRAM_SIZE];
            let (_, remote_address) =
                socket.recv_from(&mut bytes).await.map_err(|e| {
                    error!("Error receiving from socket {}", e);
                    e
                })?;

            match Message::unmarshal_binary(&mut &bytes[..]) {
                Ok(deser) => {
                    debug!("> Received raw message {}", deser.type_byte());
                    let to_process = decoder.decode(deser);
                    if let Some(message) = to_process {
                        let valid_header = PeerNode::verify_header(
                            message.header(),
                            &remote_address.ip(),
                        );
                        match valid_header {
                            true => {
                                inbound_channel_tx
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
        conf: &HashMap<String, String>,
    ) -> io::Result<()> {
        debug!("WireNetwork::listen_out started");
        let mut output_sockets = MultipleOutSocket::configure(conf).await?;
        let encoder = TransportEncoder::configure(conf);
        loop {
            if let Some((message, to)) = outbound_channel_rx.recv().await {
                debug!(
                    "< Message to send to ({:?}) - {:?} ",
                    to,
                    message.type_byte()
                );
                let chunks: Vec<Vec<u8>> =
                    encoder.encode(message).iter().map(|m| m.bytes()).collect();
                for remote_addr in to.iter() {
                    for chunk in &chunks {
                        output_sockets
                            .send(chunk, remote_addr)
                            .await
                            .unwrap_or_else(|e| {
                                error!("Unable to send msg {}", e)
                            });
                    }
                }
            }
        }
    }

    pub fn configure_socket(
        socket: &UdpSocket,
        conf: &HashMap<String, String>,
    ) -> std::io::Result<()> {
        if let Some(udp_recv_buffer_size) = conf
            .get("udp_recv_buffer_size")
            .map(|s| s.parse().ok())
            .flatten()
        {
            let sock = SockRef::from(socket);
            match sock.set_recv_buffer_size(udp_recv_buffer_size) {
                Ok(_) => {
                    info!("udp_recv_buffer is now {}", udp_recv_buffer_size)
                }
                Err(e) => {
                    error!(
                        "Error setting udp_recv_buffer to {} - {}",
                        udp_recv_buffer_size, e
                    );
                    warn!(
                        "udp_recv_buffer is still {}",
                        sock.recv_buffer_size().unwrap_or(0)
                    );
                }
            }
        }
        Ok(())
    }
}

pub fn default_configuration() -> HashMap<String, String> {
    let mut conf = TransportEncoder::default_configuration();
    conf.extend(TransportDecoder::default_configuration());
    conf.extend(MultipleOutSocket::default_configuration());
    conf.insert("udp_recv_buffer_size".to_string(), "SYSTEM".to_string());
    conf
}

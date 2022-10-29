// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Instant;

use socket2::SockRef;
use tokio::io;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::encoding::message::Message;
use crate::encoding::Marshallable;
use crate::peer::PeerNode;
use crate::rwlock::RwLock;
use crate::transport::encoding::{
    Configurable, Decoder, Encoder, TransportDecoder, TransportEncoder,
};
use crate::transport::sockets::MultipleOutSocket;

pub(crate) type MessageBeanOut = (Message, Vec<SocketAddr>);
pub(crate) type MessageBeanIn = (Message, SocketAddr);
type UDPChunk = (Vec<u8>, SocketAddr);

const MAX_DATAGRAM_SIZE: usize = 65_507;
pub(crate) struct WireNetwork {}

pub(crate) mod encoding;
pub(crate) mod sockets;

impl WireNetwork {
    pub fn start(
        inbound_channel_tx: Sender<MessageBeanIn>,
        outbound_channel_rx: Receiver<MessageBeanOut>,
        conf: Config,
        blocklist: RwLock<HashSet<SocketAddr>>,
    ) {
        let c = conf.clone();
        let (dec_chan_tx, dec_chan_rx) = mpsc::channel(conf.channel_size);

        tokio::spawn(async move {
            WireNetwork::listen_out(outbound_channel_rx, &conf).await
        });

        let c1 = c.clone();
        tokio::spawn(async move {
            WireNetwork::decode(inbound_channel_tx.clone(), dec_chan_rx, c)
                .await
        });

        tokio::spawn(async move {
            WireNetwork::listen_in(dec_chan_tx.clone(), c1, blocklist)
                .await
                .unwrap_or_else(|op| error!("Error in listen_in {:?}", op));
        });
    }

    async fn listen_in(
        dec_chan_tx: Sender<UDPChunk>,
        conf: Config,
        blocklist: RwLock<HashSet<SocketAddr>>,
    ) -> io::Result<()> {
        debug!("WireNetwork::listen_in started");

        let socket = {
            let listen_address = conf
                .listen_address
                .clone()
                .unwrap_or_else(|| conf.public_address.clone());
            UdpSocket::bind(listen_address)
                .await
                .expect("Unable to bind address")
        };
        info!("Listening on: {}", socket.local_addr()?);

        // Using a local blocklist prevent the library to constantly requests a
        // read access to the RwLock
        let last_blocklist_refresh = Instant::now();
        let blocklist_refresh = conf.network.blocklist_refresh_interval;
        let mut local_blocklist = blocklist.read().await.clone();

        // Try to extend socket recv buffer size
        WireNetwork::configure_socket(&socket, conf)?;

        // Read UDP socket recv buffer and delegate the processing to decode
        // task
        loop {
            if last_blocklist_refresh.elapsed() > blocklist_refresh {
                local_blocklist = blocklist.read().await.clone();
            }
            let mut bytes = [0; MAX_DATAGRAM_SIZE];
            let (len, remote_address) =
                socket.recv_from(&mut bytes).await.map_err(|e| {
                    error!("Error receiving from socket {}", e);
                    e
                })?;

            if local_blocklist.contains(&remote_address) {
                continue;
            }

            dec_chan_tx
                .send((bytes[0..len].to_vec(), remote_address))
                .await
                .unwrap_or_else(|op| {
                    error!("Unable to send to dec_chan_tx channel {:?}", op)
                });
        }
    }

    async fn decode(
        inbound_channel_tx: Sender<MessageBeanIn>,
        mut dec_chan_rx: Receiver<UDPChunk>,
        conf: Config,
    ) {
        debug!("WireNetwork::decode started");
        let mut decoder = TransportDecoder::configure(&conf.fec.decoder);

        loop {
            if let Some((message, remote_address)) = dec_chan_rx.recv().await {
                match Message::unmarshal_binary(&mut &message[..]) {
                    Ok(deser) => {
                        debug!("> Received raw message {}", deser.type_byte());

                        match decoder.decode(deser) {
                            Err(e) =>  error!("Unable to process the message through the decoder: {}",e),
                            Ok(Some(message)) => {
                                let valid_header = PeerNode::verify_header(
                                    message.header(),
                                    &remote_address.ip(),
                                );
                                if valid_header {
                                    inbound_channel_tx
                                        .send((message, remote_address))
                                        .await
                                        .unwrap_or_else(
                                            |op| error!("Unable to send to inbound channel {:?}", op),
                                        );
                                }
                                else {
                                    error!(
                                        "Invalid Id {:?} - {}",
                                        message.header(),
                                        &remote_address.ip()
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => error!(
                        "Error deser from {:?} - {} - {}",
                        message, remote_address, e
                    ),
                }
            }
        }
    }

    async fn listen_out(
        mut outbound_channel_rx: Receiver<MessageBeanOut>,
        conf: &Config,
    ) {
        debug!("WireNetwork::listen_out started");
        let mut output_sockets = MultipleOutSocket::configure(&conf.network);
        let encoder = TransportEncoder::configure(&conf.fec.encoder);
        loop {
            if let Some((message, to)) = outbound_channel_rx.recv().await {
                debug!(
                    "< Message to send to ({:?}) - {:?} ",
                    to,
                    message.type_byte()
                );

                match encoder.encode(message) {
                    Ok(chunks) => {
                        let chunks: Vec<Vec<u8>> = chunks
                            .iter()
                            .filter_map(|m| m.bytes().ok())
                            .collect();
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
                    Err(e) => error!("Unable to encode msg {}", e),
                }
            }
        }
    }

    pub fn configure_socket(
        socket: &UdpSocket,
        conf: Config,
    ) -> io::Result<()> {
        if let Some(udp_recv_buffer_size) = conf.network.udp_recv_buffer_size {
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

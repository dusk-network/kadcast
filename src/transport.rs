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
        in_channel_tx: Sender<MessageBeanIn>,
        out_channel_rx: Receiver<MessageBeanOut>,
        conf: Config,
        blocklist: RwLock<HashSet<SocketAddr>>,
    ) {
        let decoder = TransportDecoder::configure(&conf.fec.decoder);
        let encoder = TransportEncoder::configure(&conf.fec.encoder);
        let out_socket = MultipleOutSocket::configure(&conf.network);
        let (dec_chan_tx, dec_chan_rx) = mpsc::channel(conf.channel_size);

        let outgoing = Self::outgoing(out_channel_rx, out_socket, encoder);
        let decoder = Self::decoder(in_channel_tx, dec_chan_rx, decoder);
        let incoming = async {
            Self::incoming(dec_chan_tx, conf, blocklist)
                .await
                .unwrap_or_else(|e| error!("Error in incoming_loop {e}"));
        };

        tokio::spawn(outgoing);
        tokio::spawn(decoder);
        tokio::spawn(incoming);
    }

    async fn incoming(
        dec_chan_tx: Sender<UDPChunk>,
        conf: Config,
        blocklist: RwLock<HashSet<SocketAddr>>,
    ) -> io::Result<()> {
        debug!("WireNetwork::incoming loop started");

        let listen_address =
            conf.listen_address.as_ref().unwrap_or(&conf.public_address);
        let socket = UdpSocket::bind(listen_address).await?;

        info!("Listening on: {}", socket.local_addr()?);

        // Using a local blocklist prevent the library to constantly requests a
        // read access to the RwLock
        let last_blocklist_refresh = Instant::now();
        let blocklist_refresh = conf.network.blocklist_refresh_interval;
        let mut local_blocklist = blocklist.read().await.clone();

        // Try to extend socket recv buffer size
        Self::configure_socket(&socket, &conf)?;

        // Read UDP socket recv buffer and delegate the processing to decode
        // task
        loop {
            if last_blocklist_refresh.elapsed() > blocklist_refresh {
                local_blocklist = blocklist.read().await.clone();
            }

            let mut bytes = [0; MAX_DATAGRAM_SIZE];
            let (len, remote_address) =
                socket.recv_from(&mut bytes).await.map_err(|e| {
                    error!("Error receiving from socket {e}");
                    e
                })?;

            if local_blocklist.contains(&remote_address) {
                continue;
            }

            dec_chan_tx
                .try_send((bytes[0..len].to_vec(), remote_address))
                // .send((bytes[0..len].to_vec(), remote_address))
                // .await
                .unwrap_or_else(|e| {
                    error!("Unable to send to dec_chan_tx channel {e}")
                });
        }
    }

    async fn decoder(
        in_channel_tx: Sender<MessageBeanIn>,
        mut dec_chan_rx: Receiver<UDPChunk>,
        mut decoder: TransportDecoder,
    ) {
        debug!("WireNetwork::decoder loop started");
        loop {
            if let Some((data, src)) = dec_chan_rx.recv().await {
                match Message::unmarshal_binary(&mut &data[..]) {
                    Ok(deser) => {
                        debug!("> Received raw message {}", deser.type_byte());
                        Self::handle_raw_message(
                            &mut decoder,
                            deser,
                            src,
                            &in_channel_tx,
                        )
                        .await;
                    }
                    Err(e) => {
                        error!("Error deser from {data:?} - {src} - {e}")
                    }
                }
            }
        }
    }

    async fn handle_raw_message(
        decoder: &mut TransportDecoder,
        deser: Message,
        src: SocketAddr,
        in_channel_tx: &Sender<MessageBeanIn>,
    ) {
        match decoder.decode(deser) {
            Err(e) => {
                error!("Unable to process the message through the decoder: {e}")
            }
            Ok(Some(message)) => {
                in_channel_tx
                    .send((message, src))
                    .await
                    .unwrap_or_else(|e| {
                        error!("Unable to send to inbound channel {e}")
                    });
            }
            _ => {}
        }
    }

    async fn outgoing(
        mut out_channel_rx: Receiver<MessageBeanOut>,
        mut out_socket: MultipleOutSocket,
        encoder: TransportEncoder,
    ) {
        debug!("WireNetwork::outgoing loop started");
        loop {
            if let Some((message, targets)) = out_channel_rx.recv().await {
                debug!(
                    "< Message to send to ({targets:?}) - {:?} ",
                    message.type_byte()
                );

                match encoder.encode(message) {
                    Ok(chunks) => {
                        let chunks: Vec<_> = chunks
                            .iter()
                            .filter_map(|m| m.bytes().ok())
                            .collect();
                        for remote_addr in targets.iter() {
                            for chunk in &chunks {
                                out_socket
                                    .send(chunk, remote_addr)
                                    .await
                                    .unwrap_or_else(|e| {
                                        error!("Unable to send msg {e}")
                                    });
                            }
                        }
                    }
                    Err(e) => error!("Unable to encode msg {e}"),
                }
            }
        }
    }

    pub fn configure_socket(
        socket: &UdpSocket,
        conf: &Config,
    ) -> io::Result<()> {
        if let Some(size) = conf.network.udp_recv_buffer_size {
            let sock = SockRef::from(socket);
            match sock.set_recv_buffer_size(size) {
                Ok(_) => info!("udp_recv_buffer is now {size}"),
                Err(e) => {
                    error!("Error setting udp_recv_buffer to {size} - {e}",);
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

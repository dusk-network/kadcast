// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::runtime::Handle;
use tokio::task::block_in_place;
use tokio::time::{self, timeout, Interval};
use tracing::{info, warn};

use super::encoding::Configurable;
use crate::config::NetworkConfig;

const MIN_RETRY_COUNT: u8 = 1;

pub(super) struct MultipleOutSocket {
    ipv4: UdpSocket,
    ipv6: UdpSocket,
    udp_backoff_timeout: Option<Interval>,
    retry_count: u8,
    udp_send_retry_interval: Duration,
}

impl Configurable for MultipleOutSocket {
    type TConf = NetworkConfig;
    fn default_configuration() -> Self::TConf {
        NetworkConfig::default()
    }
    fn configure(conf: &Self::TConf) -> Self {
        let udp_backoff_timeout =
            conf.udp_send_backoff_timeout.map(time::interval);
        let retry_count = {
            match conf.udp_send_retry_count > MIN_RETRY_COUNT {
                true => conf.udp_send_retry_count,
                _ => MIN_RETRY_COUNT,
            }
        };
        let udp_send_retry_interval = conf.udp_send_retry_interval;

        let ipv4 = block_in_place(move || {
            Handle::current()
                .block_on(async move { UdpSocket::bind("0.0.0.0:0").await })
        })
        .unwrap();
        let ipv6 = block_in_place(move || {
            Handle::current()
                .block_on(async move { UdpSocket::bind("[::]:0").await })
        })
        .unwrap();
        MultipleOutSocket {
            ipv4,
            ipv6,
            udp_backoff_timeout,
            retry_count,
            udp_send_retry_interval,
        }
    }
}

impl MultipleOutSocket {
    pub(super) async fn send(
        &mut self,
        data: &[u8],
        remote_addr: &SocketAddr,
    ) -> io::Result<()> {
        if let Some(sleep) = &mut self.udp_backoff_timeout {
            sleep.tick().await;
        }
        let max_retry = self.retry_count;

        for i in 1..=max_retry {
            let send_fn = match remote_addr.is_ipv4() {
                true => self.ipv4.send_to(data, *remote_addr),
                false => self.ipv6.send_to(data, *remote_addr),
            };

            let send = timeout(self.udp_send_retry_interval, send_fn)
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "TIMEOUT"));

            match send {
                Ok(Ok(_)) => {
                    if i > 1 {
                        info!("Send msg success, attempt {i}");
                    }
                    return Ok(());
                }
                Ok(Err(e)) | Err(e) => {
                    if i < max_retry {
                        warn!("Send msg failure, attempt {i} of {max_retry}, {e}");
                        tokio::time::sleep(self.udp_send_retry_interval).await
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use tracing::error;

    use super::*;
    use crate::peer::PeerNode;
    use crate::tests::Result;
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_send_peers() -> Result<()> {
        // Generate a subscriber with the desired log level.
        let subscriber = tracing_subscriber::fmt::Subscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .finish();
        // Set the subscriber as global.
        // so this subscriber will be used as the default in all threads for the
        // remainder of the duration of the program, similar to how `loggers`
        // work in the `log` crate.
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed on subscribe tracing");
        let mut socket = MultipleOutSocket::configure(
            &MultipleOutSocket::default_configuration(),
        );
        let data = [0u8; 1000];
        let root = PeerNode::generate("192.168.0.1:666", 0)?;
        let target = root.as_peer_info().to_socket_address();

        for _ in 0..1000 * 1000 {
            if let Err(e) = socket.send(&data, &target).await {
                error!("{e}");
            }
        }
        Ok(())
    }
}

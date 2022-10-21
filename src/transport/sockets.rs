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
use tokio::time::{self, Interval};
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
        for i in 0..self.retry_count {
            let res = match remote_addr.is_ipv4() {
                true => self.ipv4.send_to(data, &remote_addr).await,
                false => self.ipv6.send_to(data, &remote_addr).await,
            };
            match res {
                Ok(_) => {
                    if i > 0 {
                        info!("Message sent, recovered from previous error");
                    }
                    return Ok(());
                }
                Err(e) => {
                    if i < (self.retry_count - 1) {
                        warn!(
                            "Unable to send msg, temptative {}/{} - {}",
                            i + 1,
                            self.retry_count,
                            e
                        );
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

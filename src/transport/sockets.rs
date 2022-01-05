// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use tokio::time::Interval;

use super::*;

const DEFAULT_BACKOFF_MICROS: u64 = 0;
const DEFAULT_RETRY_COUNT: u8 = 3;
const DEFAULT_RETRY_SLEEP_MILLIS: u64 = 5;
const MIN_RETRY_COUNT: u8 = 1;
pub(super) struct MultipleOutSocket {
    ipv4: UdpSocket,
    ipv6: UdpSocket,
    udp_backoff_timeout: Option<Interval>,
    retry_count: u8,
    udp_send_retry_interval: Duration,
}

impl MultipleOutSocket {
    pub(crate) fn default_configuration() -> HashMap<String, String> {
        let mut conf = HashMap::new();
        conf.insert(
            "udp_backoff_timeout_micros".to_string(),
            DEFAULT_BACKOFF_MICROS.to_string(),
        );
        conf.insert(
            "udp_send_retry_count".to_string(),
            DEFAULT_RETRY_COUNT.to_string(),
        );
        conf.insert(
            "udp_send_retry_interval_millis".to_string(),
            DEFAULT_RETRY_SLEEP_MILLIS.to_string(),
        );
        conf
    }
    pub(crate) async fn configure(
        conf: &HashMap<String, String>,
    ) -> io::Result<Self> {
        let udp_backoff_timeout = {
            let micros = conf
                .get("udp_backoff_timeout_micros")
                .map(|s| s.parse().ok())
                .flatten()
                .unwrap_or(DEFAULT_BACKOFF_MICROS);
            match micros > 0 {
                true => Some(time::interval(Duration::from_micros(micros))),
                _ => None,
            }
        };

        let udp_send_retry_interval = Duration::from_millis(
            conf.get("udp_send_retry_interval_millis")
                .map(|s| s.parse().ok())
                .flatten()
                .unwrap_or(DEFAULT_RETRY_SLEEP_MILLIS),
        );
        let retry_count = {
            let retry = conf
                .get("udp_send_retry_count")
                .map(|s| s.parse().ok())
                .flatten()
                .unwrap_or(DEFAULT_RETRY_COUNT);
            match retry > MIN_RETRY_COUNT {
                true => retry,
                _ => MIN_RETRY_COUNT,
            }
        };

        let ipv4 = UdpSocket::bind("0.0.0.0:0").await?;
        let ipv6 = UdpSocket::bind("[::]:0").await?;
        Ok(MultipleOutSocket {
            ipv4,
            ipv6,
            udp_backoff_timeout,
            retry_count,
            udp_send_retry_interval,
        })
    }

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
        // Err(e)=>
        // Ok(())
    }
}

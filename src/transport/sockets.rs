// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use tokio::time::Interval;

use super::*;

const DEFAULT_BACKOFF_MICROS: u64 = 0;
pub(super) struct MultipleOutSocket {
    ipv4: UdpSocket,
    ipv6: UdpSocket,
    udp_backoff_timeout: Option<Interval>,
}

impl Configurable for MultipleOutSocket {
    fn configure(conf: &HashMap<String, String>) -> Self {
        Runtime::new()
            .unwrap()
            .block_on(MultipleOutSocket::new(conf))
            .unwrap()
    }
    fn default_configuration() -> HashMap<String, String> {
        let mut conf = HashMap::new();
        conf.insert(
            "udp_backoff_timeout_micros".to_string(),
            DEFAULT_BACKOFF_MICROS.to_string(),
        );
        conf
    }
}

impl MultipleOutSocket {
    async fn new(conf: &HashMap<String, String>) -> io::Result<Self> {
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

        let ipv4 = UdpSocket::bind("0.0.0.0:0").await?;
        let ipv6 = UdpSocket::bind("[::]:0").await?;
        Ok(MultipleOutSocket {
            ipv4,
            ipv6,
            udp_backoff_timeout,
        })
    }
    pub(super) async fn send(
        &mut self,
        data: &[u8],
        remote_addr: &SocketAddr,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(sleep) = &mut self.udp_backoff_timeout {
            sleep.tick().await;
        }
        match remote_addr.is_ipv4() {
            true => self.ipv4.send_to(data, &remote_addr).await?,
            false => self.ipv6.send_to(data, &remote_addr).await?,
        };
        Ok(())
    }
}

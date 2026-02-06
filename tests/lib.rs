// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

#[cfg(test)]
mod tests {

    use std::collections::{HashMap, HashSet};
    use std::net::{AddrParseError, SocketAddr, ToSocketAddrs};
    use std::ops::Range;
    use std::time::Duration;

    use kadcast::config::Config;
    use kadcast::{MessageInfo, NetworkListen, Peer};
    use tokio::{sync::mpsc, time::timeout};
    use tracing::{info, warn};

    const NODES: i32 = 10;
    const BASE_PORT: i32 = 20000;
    const BOOTSTRAP_COUNT: i32 = 3; // the first 2 bootstrappers are on different chain/version
    const WAIT_SEC: u64 = 20;
    const MESSAGE_SIZE: usize = 100_000;

    #[test]
    fn test_dns_resolver() {
        let server_details = "192.168.1.5:80";
        let server: Vec<_> = server_details
            .to_socket_addrs()
            .expect("To resolve domain")
            .collect();
        println!("{:?}", server);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn harness_test() -> Result<(), Box<dyn std::error::Error>> {
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
        let (tx, rx) = mpsc::channel(100);

        let bootstraps = {
            let mut v = vec![];
            for i in 0..BOOTSTRAP_COUNT {
                v.push(format!("127.0.0.1:{}", BASE_PORT + i).to_string());
            }
            v
        };
        let mut peers = HashMap::new();

        // Use a wrong network_id for the first bootstrapper
        peers.insert(
            0,
            create_peer(0, bootstraps.clone(), tx.clone(), Some(2))?,
        );
        for i in 1..NODES {
            tokio::time::sleep(Duration::from_millis(500)).await;
            peers.insert(
                i,
                create_peer(i, bootstraps.clone(), tx.clone(), Some(1))?,
            );
        }

        tokio::time::sleep(Duration::from_millis(2000)).await;
        let mut data: Vec<u8> = vec![0; MESSAGE_SIZE];
        for i in 0..data.len() {
            data[i] = rand::Rng::r#gen(&mut rand::thread_rng());
        }
        for i in 0..NODES {
            info!("ROUTING TABLE PEER #{}", i);
            peers.get(&i).unwrap().report().await;
            info!("----------------------");
            info!("FIRST 20 ALIVE ADDRESSES FOR #{}", i);
            for s in peers.get(&i).unwrap().alive_nodes(20).await {
                info!("{}", s);
            }
            info!("----------------------");
        }

        peers
            .get(&(NODES - 1))
            .unwrap()
            .broadcast(&data, None)
            .await;
        let expected_message_broadcasted = NODES;
        // Remove the invalid network id
        let expected_message_sent = expected_message_broadcasted - 1;
        // Remove the sender
        let expected_message_received = expected_message_sent - 2;

        // remove the first two invalid nodes
        // First one has invalid ChainID
        // Second one has invalid Version
        let start_expected_range = BASE_PORT + 2;
        let end_expected_range =
            start_expected_range + expected_message_received;

        let expected_received_range = start_expected_range..end_expected_range;

        let res = timeout(
            Duration::from_secs(WAIT_SEC),
            receive(rx, expected_received_range),
        )
        .await;
        assert!(
            res.is_ok(),
            "Not all nodes received the broadcasted message"
        );
        Ok(())
    }

    async fn receive(
        mut rx: mpsc::Receiver<(usize, (Vec<u8>, SocketAddr, u8))>,
        expected_from: Range<i32>,
    ) {
        let mut missing = HashSet::new();
        info!("expected_from: {expected_from:?}");
        for i in expected_from {
            missing.insert(i);
        }
        info!("{missing:?}");
        let mut i = 0;
        while !missing.is_empty() {
            if let Some((receiver_port, message)) = rx.recv().await {
                i = i + 1;
                let removed = missing.remove(&(receiver_port as i32));
                info!(
                    "RECEIVER PORT: {} - Message N° {} got from {:?} -  Left {} - Removed {:?}",
                    receiver_port, i, message.1, missing.len(), removed
                );
            }
        }
        info!("Received All {} messages", i);
        info!("missing: {missing:?}");
    }

    fn create_peer(
        i: i32,
        bootstrap: Vec<String>,
        grpc_sender: mpsc::Sender<(usize, (Vec<u8>, SocketAddr, u8))>,
        network_id: Option<u8>,
    ) -> core::result::Result<Peer, AddrParseError> {
        let port = BASE_PORT + i;
        let public_addr = format!("127.0.0.1:{port}");
        let listener = KadcastListener {
            grpc_sender,
            receiver_port: port as usize,
        };
        let mut conf = Config::default();
        conf.kadcast_id = network_id;
        conf.bootstrapping_nodes = bootstrap;
        conf.public_address = public_addr;
        conf.version_match = ">=1.2.2".to_string();
        conf.version = format!("1.2.{i}");
        conf.recursive_discovery = false;
        Peer::new(conf, listener)
    }

    struct KadcastListener {
        grpc_sender: mpsc::Sender<(usize, (Vec<u8>, SocketAddr, u8))>,
        receiver_port: usize,
    }

    impl NetworkListen for KadcastListener {
        fn on_message(&self, message: Vec<u8>, metadata: MessageInfo) {
            self.grpc_sender
                .try_send((
                    self.receiver_port,
                    (message, metadata.src(), metadata.height()),
                ))
                .unwrap_or_else(|e| {
                    warn!("Error sending to listener {}", e);
                });
        }
    }
}

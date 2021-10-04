#[cfg(test)]
mod tests {
    use p2p::{KadcastBroadcastListener, KadcastProtoBuilder};

    #[test]
    fn libtest() {
        let public_ip = "127.0.0.1:666";
        let bootstrapping_nodes =
            vec!["127.0.0.1:10000".to_string(), "127.0.0.1:20000".to_string()];
        let mut proto = KadcastProtoBuilder::new(public_ip, bootstrapping_nodes)
            .set_broadcast_listener(DummyListener {})
            .build();
        proto.bootstrap();
        proto.send_message(vec![0u8, 1u8]);
    }

    struct DummyListener {}
    impl KadcastBroadcastListener for DummyListener {
        fn on_broadcast(&self, _message: Vec<u8>) {}
    }
}

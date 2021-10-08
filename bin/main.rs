use p2p::KadcastServerBuilder;

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    let public_ip = "127.0.0.1:2000";
    let bootstrapping_nodes = vec![
        "voucher.dusk.network:555".to_string(),
        "voucher2.dusk.network:555".to_string(),
    ];
    let server = KadcastServerBuilder::new(public_ip, bootstrapping_nodes).build();
    server.bootstrap().await;
}

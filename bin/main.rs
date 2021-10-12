use tokio::time::{sleep, Duration};

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    let public_ip = "127.0.0.1:2000";
    let bootstrapping_nodes = vec![
        "voucher.dusk.network:555".to_string(),
        "voucher2.dusk.network:555".to_string(),
    ];
    let server = kadcast::Server::new(public_ip.to_string(), bootstrapping_nodes);
    async move {
        loop {
            sleep(Duration::from_millis(1000)).await;
            server.broadcast(vec![0u8]).await;
        }
    }
    .await;
}

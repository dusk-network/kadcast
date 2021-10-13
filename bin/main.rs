use clap::{App, Arg};
use rustc_tools_util::{get_version_info, VersionInfo};
use std::io::{self, BufRead};

use crate::version::show_version;
mod version;
#[tokio::main]
pub async fn main() {
    let crate_info = get_version_info!();
    let matches = App::new(&crate_info.crate_name)
        .version(show_version(crate_info).as_str())
        .author("Dusk Network B.V. All Rights Reserved.")
        .about("Kadcast Network impl.")
        .arg(
            Arg::with_name("host")
                .short("h")
                .long("host")
                .help("Address you want to use")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("bootstrap")
                .long("bootstrap")
                .short("b")
                .multiple(true)
                .help("List of bootstrapping server instances")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let public_ip = matches.value_of("host").unwrap();
    println!("{:?}", public_ip);
    let bootstrapping_nodes = matches
        .values_of("bootstrap")
        .unwrap_or_default()
        .map(|s| s.to_string())
        .collect();

    println!("{:?}", bootstrapping_nodes);
    let server = kadcast::Server::new(public_ip.to_string(), bootstrapping_nodes);
    loop {
        let stdin = io::stdin();
        for message in stdin.lock().lines().flatten() {
            match &message[..] {
                "report" => {
                    server.report().await;
                }
                v => server.broadcast(v.as_bytes().to_vec()).await,
            }
        }
    }
}

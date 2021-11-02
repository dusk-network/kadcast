// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use clap::{App, Arg};
use kadcast::Peer;
use rustc_tools_util::{get_version_info, VersionInfo};
use std::io::{self, BufRead};

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
        .arg(
            Arg::with_name("log-level")
                .long("log-level")
                .value_name("LOG")
                .possible_values(&["error", "warn", "info", "debug", "trace"])
                .default_value("info")
                .help("Output log level")
                .takes_value(true),
        )
        .get_matches();

    let public_ip = matches.value_of("host").unwrap();
    let bootstrapping_nodes = matches
        .values_of("bootstrap")
        .unwrap_or_default()
        .map(|s| s.to_string())
        .collect();

    // Match tracing desired level.
    let log = match matches
        .value_of("log-level")
        .expect("Failed parsing log-level arg")
    {
        "error" => tracing::Level::ERROR,
        "warn" => tracing::Level::WARN,
        "info" => tracing::Level::INFO,
        "debug" => tracing::Level::DEBUG,
        "trace" => tracing::Level::TRACE,
        _ => unreachable!(),
    };

    // Generate a subscriber with the desired log level.
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_max_level(log)
        .finish();
    // Set the subscriber as global.
    // so this subscriber will be used as the default in all threads for the
    // remainder of the duration of the program, similar to how `loggers`
    // work in the `log` crate.
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed on subscribe tracing");

    let peer = Peer::builder(
        public_ip.to_string(),
        bootstrapping_nodes,
        crate::on_message,
    )
    .build();
    loop {
        let stdin = io::stdin();
        for message in stdin.lock().lines().flatten() {
            match &message[..] {
                "report" => {
                    peer.report().await;
                }
                v => peer.broadcast(v.as_bytes()).await,
            }
        }
    }
}

fn on_message(message: Vec<u8>) {
    println!(
        "Received {}",
        String::from_utf8(message.to_vec())
            .unwrap_or_else(|_| "No UTF8 message received".to_string())
    );
}

fn show_version(info: VersionInfo) -> String {
    let version = format!("{}.{}.{}", info.major, info.minor, info.patch);
    let build = format!(
        "{} {}",
        info.commit_hash.unwrap_or_default(),
        info.commit_date.unwrap_or_default()
    );

    if build.len() > 1 {
        format!("{} ({})", version, build)
    } else {
        version
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::time::Duration;

use serde_derive::{Deserialize, Serialize};

use crate::transport::encoding::{
    Configurable, TransportDecoder, TransportEncoder,
};
pub use crate::transport::encoding::{
    TransportDecoderConfig, TransportEncoderConfig,
};

/// Default value while a node is considered alive (no eviction will be
/// requested)
pub const BUCKET_DEFAULT_NODE_TTL_MILLIS: u64 = 30000;

/// Default value after which a node can be evicted if requested
pub const BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS: u64 = 5000;

/// Default value after which a bucket is considered idle
pub const BUCKET_DEFAULT_TTL_SECS: u64 = 60 * 60;

/// Default behaviour for propagation of incoming broadcast messages
pub const ENABLE_BROADCAST_PROPAGATION: bool = true;

/// Default internal channel size
pub const DEFAULT_CHANNEL_SIZE: usize = 1000;

pub const DEFAULT_SEND_RETRY_COUNT: u8 = 3;
pub const DEFAULT_SEND_RETRY_SLEEP_MILLIS: u64 = 5;
pub const DEFAULT_BLOCKLIST_REFRESH_SECS: u64 = 10;

/// Default minimum peers required for network integration without bootstrapping
pub const DEFAULT_MIN_PEERS_FOR_INTEGRATION: usize = 3;

const DEFAULT_VERSION: &str = "0.0.1";
const DEFAULT_VERSION_MATCH: &str = "*";

const fn default_min_peers() -> usize {
    DEFAULT_MIN_PEERS_FOR_INTEGRATION
}

fn default_version() -> String {
    DEFAULT_VERSION.to_string()
}

fn default_version_match() -> String {
    DEFAULT_VERSION_MATCH.to_string()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    /// KadcastID
    pub kadcast_id: Option<u8>,

    /// Public `SocketAddress` of the [Peer]. No domain name allowed
    ///
    /// This is the address where other peers can contact you.
    /// This address MUST be accessible from any peer of the network
    pub public_address: String,

    /// Optional internal `SocketAddress` to listen incoming connections.
    /// Eg: 127.0.0.1:9999
    ///
    /// This address is the one bound for the incoming connections.
    /// Use it if your host is not publicly reachable from other peer in the
    /// network (Eg: if you are behind a NAT)
    /// If this is not specified, the public address will be used for binding
    /// incoming connection
    pub listen_address: Option<String>,

    /// List of known bootstrapping kadcast nodes.
    ///
    /// It accepts the same representation of `public_address` but with domain
    /// names allowed
    pub bootstrapping_nodes: Vec<String>,

    /// Enable automatic propagation of incoming broadcast messages
    ///
    /// Default value [ENABLE_BROADCAST_PROPAGATION]
    pub auto_propagate: bool,
    pub channel_size: usize,

    /// Send a `FindNodes` message to every Peer inside `Nodes` message
    /// received
    ///
    /// If disabled, a `Ping` is sent instead of `FindNodes`
    /// Default value `true]`
    pub recursive_discovery: bool,

    /// Buckets configuration
    pub bucket: BucketConfig,

    /// Network configuration
    pub network: NetworkConfig,

    /// FEC configuration
    pub fec: FECConfig,

    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_version_match")]
    pub version_match: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            public_address: "127.0.0.1:9000".to_string(),
            kadcast_id: None,
            listen_address: None,
            bootstrapping_nodes: vec![],
            auto_propagate: ENABLE_BROADCAST_PROPAGATION,
            channel_size: DEFAULT_CHANNEL_SIZE,
            recursive_discovery: true,
            network: NetworkConfig::default(),
            bucket: BucketConfig::default(),
            fec: FECConfig::default(),
            version: default_version(),
            version_match: default_version_match(),
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct BucketConfig {
    /// Sets the maximum duration for a node to be considered alive (no
    /// eviction will be requested).
    ///
    /// Default value [BUCKET_DEFAULT_NODE_TTL_MILLIS]
    #[serde(with = "humantime_serde")]
    pub node_ttl: Duration,

    /// Set duration after which a node can be evicted if requested
    ///
    /// Default value [BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS]
    #[serde(with = "humantime_serde")]
    pub node_evict_after: Duration,

    /// Set duration after which a bucket is considered idle
    ///
    /// Default value [BUCKET_DEFAULT_TTL_SECS]
    #[serde(with = "humantime_serde")]
    pub bucket_ttl: Duration,

    /// Minimum number of nodes at which bootstrapping is not required
    ///
    /// This value represents the minimum number of nodes required for a peer
    /// to consider bootstrapping unnecessary and seamlessly integrate into
    /// the existing network.
    ///
    /// Default value [DEFAULT_MIN_PEERS_FOR_INTEGRATION]
    #[serde(default = "default_min_peers")]
    pub min_peers: usize,
}

impl Default for BucketConfig {
    fn default() -> Self {
        Self {
            node_evict_after: Duration::from_millis(
                BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS,
            ),
            node_ttl: Duration::from_millis(BUCKET_DEFAULT_NODE_TTL_MILLIS),
            bucket_ttl: Duration::from_secs(BUCKET_DEFAULT_TTL_SECS),
            min_peers: default_min_peers(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct FECConfig {
    pub encoder: TransportEncoderConfig,
    pub decoder: TransportDecoderConfig,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct NetworkConfig {
    pub udp_recv_buffer_size: Option<usize>,
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    pub udp_send_backoff_timeout: Option<Duration>,
    #[serde(with = "humantime_serde")]
    pub udp_send_retry_interval: Duration,
    pub udp_send_retry_count: u8,
    #[serde(with = "humantime_serde")]
    pub blocklist_refresh_interval: Duration,
}

impl Default for FECConfig {
    fn default() -> Self {
        Self {
            encoder: TransportEncoder::default_configuration(),
            decoder: TransportDecoder::default_configuration(),
        }
    }
}
impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            udp_recv_buffer_size: Some(5000000),
            udp_send_backoff_timeout: None,
            udp_send_retry_interval: Duration::from_millis(
                DEFAULT_SEND_RETRY_SLEEP_MILLIS,
            ),
            udp_send_retry_count: DEFAULT_SEND_RETRY_COUNT,
            blocklist_refresh_interval: Duration::from_secs(
                DEFAULT_BLOCKLIST_REFRESH_SECS,
            ),
        }
    }
}

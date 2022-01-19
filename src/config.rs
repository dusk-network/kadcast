// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::transport::encoding::BaseConfigurable;
use crate::transport::encoding::TransportDecoder;
pub use crate::transport::encoding::TransportDecoderConfig;
use crate::transport::encoding::TransportEncoder;
pub use crate::transport::encoding::TransportEncoderConfig;
use crate::transport::sockets::DEFAULT_RETRY_COUNT;
use crate::transport::sockets::DEFAULT_RETRY_SLEEP_MILLIS;
use serde_derive::{Deserialize, Serialize};
use std::time::Duration;

/// Default value while a node is considered alive (no eviction will be
/// requested)
pub const BUCKET_DEFAULT_NODE_TTL_MILLIS: u64 = 30000;

/// Default value after which a node can be evicted if requested
pub const BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS: u64 = 5000;

/// Default value after which a bucket is considered idle
pub const BUCKET_DEFAULT_TTL_SECS: u64 = 60 * 60;

/// Default behaviour for propagation of incoming broadcast messages
pub const ENABLE_BROADCAST_PROPAGATION: bool = true;

const DEFAULT_CHANNEL_SIZE: usize = 1000;

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub public_address: String,
    pub listen_address: Option<String>,
    pub bootstrapping_nodes: Vec<String>,
    pub auto_propagate: bool,
    pub channel_size: usize,
    pub recursive_discovery: bool,
    pub bucket: BucketConfig,
    pub network: NetworkConfig,
    pub fec: FECConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            public_address: "127.0.0.1:9000".to_string(),
            listen_address: None,
            bootstrapping_nodes: vec![],
            auto_propagate: ENABLE_BROADCAST_PROPAGATION,
            channel_size: DEFAULT_CHANNEL_SIZE,
            recursive_discovery: true,
            network: NetworkConfig::default(),
            bucket: BucketConfig::default(),
            fec: FECConfig::default(),
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct BucketConfig {
    #[serde(with = "humantime_serde")]
    pub node_ttl: Duration,
    #[serde(with = "humantime_serde")]
    pub node_evict_after: Duration,
    #[serde(with = "humantime_serde")]
    pub bucket_ttl: Duration,
}

impl Default for BucketConfig {
    fn default() -> Self {
        Self {
            node_evict_after: Duration::from_millis(
                BUCKET_DEFAULT_NODE_EVICT_AFTER_MILLIS,
            ),
            node_ttl: Duration::from_millis(BUCKET_DEFAULT_NODE_TTL_MILLIS),
            bucket_ttl: Duration::from_secs(BUCKET_DEFAULT_TTL_SECS),
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
    #[serde(with = "humantime_serde")]
    pub udp_send_backoff_timeout: Option<Duration>,
    #[serde(with = "humantime_serde")]
    pub udp_send_retry_interval: Duration,
    pub udp_send_retry_count: u8,
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
            udp_send_backoff_timeout: Some(Duration::from_micros(50)),
            udp_send_retry_interval: Duration::from_millis(
                DEFAULT_RETRY_SLEEP_MILLIS,
            ),
            udp_send_retry_count: DEFAULT_RETRY_COUNT,
        }
    }
}

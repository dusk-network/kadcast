// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::kbucket::{BinaryID, BinaryKey};
use blake2::{Blake2s, Digest};
use std::convert::TryInto;
use std::net::{IpAddr, SocketAddr};
pub type PeerNode = Node<PeerInfo>;
use crate::encoding::message::Header;
use crate::encoding::payload::{IpInfo, PeerEncodedInfo};

use crate::kbucket::Node;
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PeerInfo {
    address: SocketAddr,
}

impl PeerInfo {
    pub fn address(&self) -> &SocketAddr {
        &self.address
    }
}

impl PeerNode {
    pub fn from_address(address: &str) -> Self {
        let server: SocketAddr =
            address.parse().expect("Unable to parse address");
        PeerNode::from_socket(server)
    }

    pub fn from_socket(server: SocketAddr) -> Self {
        let info = PeerInfo { address: server };
        let binary =
            PeerNode::compute_id(&info.address.ip(), info.address.port());
        let id = BinaryID::new(binary);
        Node::new(id, info)
    }

    pub(crate) fn verify_header(header: &Header, ip: &IpAddr) -> bool {
        *header.binary_id.as_binary()
            == PeerNode::compute_id(ip, header.sender_port)
    }

    pub(crate) fn compute_id(ip: &IpAddr, port: u16) -> BinaryKey {
        let mut hasher = Blake2s::new();
        hasher.update(port.to_le_bytes());
        match ip {
            IpAddr::V4(ip) => hasher.update(ip.octets()),
            IpAddr::V6(ip) => hasher.update(ip.octets()),
        };
        let a: [u8; 32] = hasher
            .finalize()
            .as_slice()
            .try_into()
            .expect("Wrong length");
        let mut x = vec![0u8; 16];
        x.clone_from_slice(&a[..16]);
        x.try_into().expect("Wrong length")
    }

    //TODO: demote me as pub(crate) so this method will be used internally by
    // the protocol itself
    pub fn as_header(&self) -> Header {
        Header {
            binary_id: *self.id(),
            sender_port: self.value().address.port(),
            reserved: [0; 2],
        }
    }

    pub(crate) fn as_peer_info(&self) -> PeerEncodedInfo {
        PeerEncodedInfo {
            id: *self.id().as_binary(),
            ip: match self.value().address.ip() {
                IpAddr::V4(ip) => IpInfo::IPv4(ip.octets()),
                IpAddr::V6(ip) => IpInfo::IPv6(ip.octets()),
            },
            port: self.value().address.port(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::peer::PeerNode;

    #[test]
    fn verify_header() {
        let wrong_header = PeerNode::from_address("10.0.0.1:333").as_header();
        let wrong_header_sameport =
            PeerNode::from_address("10.0.0.1:666").as_header();
        vec![
            PeerNode::from_address("192.168.1.1:666"),
            PeerNode::from_address(
                "[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:666",
            ),
        ]
        .iter()
        .for_each(|n| {
            let ip = &n.value().address.ip();
            PeerNode::verify_header(&n.as_header(), ip);
            assert!(PeerNode::verify_header(&n.as_header(), ip));
            assert!(!PeerNode::verify_header(&wrong_header, ip));
            assert!(!PeerNode::verify_header(&wrong_header_sameport, ip));
        });
    }
}

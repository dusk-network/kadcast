// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::convert::TryInto;
use std::net::{AddrParseError, IpAddr, SocketAddr};

use blake2::{Blake2s256, Digest};

use crate::kbucket::{BinaryID, BinaryKey};
pub type PeerNode = Node<PeerInfo>;
use crate::encoding::message::Header;
use crate::encoding::payload::{IpInfo, PeerEncodedInfo};
use crate::kbucket::Node;
use crate::K_ID_LEN_BYTES;
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
    pub fn generate(address: &str) -> Result<Self, AddrParseError> {
        let address: SocketAddr = address.parse()?;
        let info = PeerInfo { address };
        let binary =
            PeerNode::compute_id(&info.address.ip(), info.address.port());
        let id = BinaryID::generate(binary);
        Ok(Node::new(id, info))
    }

    pub fn from_socket(address: SocketAddr, id: BinaryID) -> Self {
        let info = PeerInfo { address };
        Node::new(id, info)
    }

    pub(crate) fn verify_header(header: &Header, ip: &IpAddr) -> bool {
        *header.binary_id.as_binary()
            == PeerNode::compute_id(ip, header.sender_port)
    }

    pub(crate) fn compute_id(ip: &IpAddr, port: u16) -> BinaryKey {
        let mut hasher = Blake2s256::new();
        hasher.update(port.to_le_bytes());
        match ip {
            IpAddr::V4(ip) => hasher.update(ip.octets()),
            IpAddr::V6(ip) => hasher.update(ip.octets()),
        };
        let hash = hasher.finalize();
        hash[..K_ID_LEN_BYTES]
            .try_into()
            .expect("compute_id length = K_ID_LEN_BYTES")
    }

    pub(crate) fn as_header(&self) -> Header {
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
    use crate::tests::Result;
    #[test]
    fn test_verify_header() -> Result<()> {
        let wrong_header = PeerNode::generate("10.0.0.1:333")?.as_header();
        let wrong_header_sameport =
            PeerNode::generate("10.0.0.1:666")?.as_header();
        vec![
            PeerNode::generate("192.168.1.1:666")?,
            PeerNode::generate(
                "[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:666",
            )?,
        ]
        .iter()
        .for_each(|n| {
            let ip = &n.value().address.ip();
            PeerNode::verify_header(&n.as_header(), ip);
            assert!(PeerNode::verify_header(&n.as_header(), ip));
            assert!(!PeerNode::verify_header(&wrong_header, ip));
            assert!(!PeerNode::verify_header(&wrong_header_sameport, ip));
        });
        Ok(())
    }
}

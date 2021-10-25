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
        let server: SocketAddr = address.parse().expect("Unable to parse address");
        PeerNode::from_socket(server)
    }

    pub fn from_socket(server: SocketAddr) -> Self {
        let info = PeerInfo { address: server };
        let binary = PeerNode::compute_id(&info);
        let id = BinaryID::new(binary);
        Node::new(id, info)
    }

    fn compute_id(info: &PeerInfo) -> BinaryKey {
        let mut hasher = Blake2s::new();
        hasher.update(info.address.port().to_le_bytes());
        match info.address.ip() {
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

    //TODO: demote me as pub(crate) so this method will be used internally by the protocol itself
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

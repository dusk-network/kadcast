pub use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use bytes::Bytes;

use crate::kbucket::{BinaryID, Node};

pub struct PeerInfo {
    address: SocketAddr,
}

pub struct PeerID {
    str_id: String,
    binary: Bytes,
}

impl PeerID {
    fn new(address: String) -> PeerID {
        PeerID {
            str_id: address,
            binary: Bytes::new(), //TODO
        }
    }
}

impl BinaryID for PeerID {
    fn as_binary(&self) -> bytes::Bytes {
        self.binary.clone()
    }
}

pub fn from_address(address: String) -> Node<PeerID, PeerInfo> {
    let server: SocketAddr = address.parse().expect("Unable to parse address");
    Node::new(PeerID::new(address), PeerInfo { address: server })
}

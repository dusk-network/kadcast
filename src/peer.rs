use std::convert::TryInto;
pub use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use blake2::{Blake2s, Digest};

use crate::{
    kbucket::{BinaryID, Node},
    K_DIFF_MIN, K_DIFF_PRODUCED, K_ID_LEN_BYTES, K_NONCE_LEN,
};

pub struct PeerInfo {
    address: SocketAddr,
}

pub struct PeerID {
    str_id: String,
    binary: [u8; K_ID_LEN_BYTES],
    nonce: [u8; K_NONCE_LEN],
}

impl PeerID {
    fn compute_id(info: &PeerInfo) -> [u8; K_ID_LEN_BYTES] {
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
}

impl BinaryID for PeerID {
    fn as_binary(&self) -> &[u8; K_ID_LEN_BYTES] {
        &self.binary
    }

    fn nonce(&self) -> &[u8; K_NONCE_LEN] {
        &self.nonce
    }
}

pub fn from_address(address: String) -> Node<PeerID, PeerInfo> {
    let server: SocketAddr = address.parse().expect("Unable to parse address");
    let info = PeerInfo { address: server };
    let binary_id = PeerID::compute_id(&info);
    let id = PeerID {
        str_id: address,
        binary: binary_id,
        nonce: compute_nonce(&binary_id),
    };
    PeerID::compute_id(&info);
    Node::new(id, info)
}

pub fn compute_nonce(id: &[u8; K_ID_LEN_BYTES]) -> [u8; K_NONCE_LEN] {
    let mut nonce: u32 = 0;
    let mut hasher = Blake2s::new();
    loop {
        hasher.update(id);
        let nonce_bytes = nonce.to_le_bytes();
        hasher.update(nonce_bytes);
        let hash = hasher.finalize_reset();
        if hash.into_iter().rev().take(K_DIFF_PRODUCED).all(|n| n == 0) {
            return nonce_bytes;
        }
        nonce += 1;
    }
}

pub fn verify_nonce(id: &[u8; K_ID_LEN_BYTES], nonce: &[u8; K_NONCE_LEN]) -> bool {
    let mut hasher = Blake2s::new();
    hasher.update(id);
    hasher.update(nonce);
    hasher
        .finalize()
        .into_iter()
        .rev()
        .take(K_DIFF_MIN)
        .all(|n| n == 0)
}

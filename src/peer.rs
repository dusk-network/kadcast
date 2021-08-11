pub use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::{convert::TryInto, slice::Iter};

use blake2::{Blake2s, Digest};

use crate::{
    kbucket::{BinaryID, Node},
    K_DIFF_MIN_BIT, K_DIFF_PRODUCED_BIT, K_ID_LEN_BYTES, K_NONCE_LEN,
};
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PeerInfo {
    address: SocketAddr,
}
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
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
    assert!(
        K_DIFF_PRODUCED_BIT >= K_DIFF_MIN_BIT,
        "PoW is less than minimum required, review your config..."
    );
    let mut nonce: u32 = 0;
    let mut hasher = Blake2s::new();
    loop {
        hasher.update(id);
        let nonce_bytes = nonce.to_le_bytes();
        hasher.update(nonce_bytes);
        if verify_difficulty(
            &mut hasher.finalize_reset().iter().rev(),
            K_DIFF_PRODUCED_BIT,
        ) {
            return nonce_bytes;
        }
        nonce += 1;
    }
}

pub fn verify_nonce(id: &[u8; K_ID_LEN_BYTES], nonce: &[u8; K_NONCE_LEN]) -> bool {
    let mut hasher = Blake2s::new();
    hasher.update(id);
    hasher.update(nonce);
    verify_difficulty(&mut hasher.finalize().iter().rev(), K_DIFF_MIN_BIT)
}

fn verify_difficulty<'a, I>(bytes: &mut I, difficulty: usize) -> bool
where
    I: Iterator<Item = &'a u8>,
{
    bytes.next().map_or(false, |b| {
        if difficulty <= 8 {
            b.trailing_zeros() as usize >= difficulty
        } else {
            if b != &0 {
                false
            } else {
                verify_difficulty(bytes, difficulty - 8)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::peer::verify_difficulty;

    #[test]
    fn test_difficulty() {
        let b: [u8; 2] = [0b11110000, 0b00000000];
        assert!(verify_difficulty(&mut b.iter().rev(), 0));
        assert!(verify_difficulty(&mut b.iter().rev(), 1));
        assert!(verify_difficulty(&mut b.iter().rev(), 2));
        assert!(verify_difficulty(&mut b.iter().rev(), 3));
        assert!(verify_difficulty(&mut b.iter().rev(), 4));
        assert!(verify_difficulty(&mut b.iter().rev(), 5));
        assert!(verify_difficulty(&mut b.iter().rev(), 6));
        assert!(verify_difficulty(&mut b.iter().rev(), 7));
        assert!(verify_difficulty(&mut b.iter().rev(), 8));
        assert!(verify_difficulty(&mut b.iter().rev(), 9));
        assert!(verify_difficulty(&mut b.iter().rev(), 10));
        assert!(verify_difficulty(&mut b.iter().rev(), 11));
        assert!(verify_difficulty(&mut b.iter().rev(), 12));
        assert!(!verify_difficulty(&mut b.iter().rev(), 13));
    }
}

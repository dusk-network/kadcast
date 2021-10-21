use crate::K_ID_LEN_BYTES;
use crate::K_NONCE_LEN;

pub type BinaryKey = [u8; K_ID_LEN_BYTES];
pub type BinaryNonce = [u8; K_NONCE_LEN];

use blake2::{Blake2s, Digest};

use crate::{K_DIFF_MIN_BIT, K_DIFF_PRODUCED_BIT};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct BinaryID {
    bytes: BinaryKey,
    nonce: BinaryNonce,
}

impl BinaryID {
    pub fn as_binary(&self) -> &BinaryKey {
        &self.bytes
    }
    pub fn nonce(&self) -> &BinaryNonce {
        &self.nonce
    }

    // Returns the 0-based kadcast distance between 2 ID
    // `None` if they are identical
    pub fn calculate_distance(&self, other: &BinaryID) -> Option<usize> {
        self.as_binary()
            .iter()
            .zip(other.as_binary().iter())
            .map(|(&a, &b)| a ^ b)
            .enumerate()
            .rev()
            .find(|(_, b)| b != &0b0)
            .map(|(i, b)| BinaryID::msb(b).expect("Can't be None") + (i << 3) - 1)
    }

    // Returns the position of the most-significant bit set in a byte,
    // `None` if no bit is set
    const fn msb(n: u8) -> Option<usize> {
        match u8::BITS - n.leading_zeros() {
            0 => None,
            n => Some(n as usize),
        }
    }

    pub(crate) fn from_nonce(id: BinaryKey, nonce: BinaryNonce) -> Self {
        BinaryID { bytes: id, nonce }
    }

    pub(crate) fn new(id: BinaryKey) -> Self {
        if K_DIFF_PRODUCED_BIT < K_DIFF_MIN_BIT {
            panic!("PoW is less than minimum required, review your build config...")
        }
        let mut nonce: u32 = 0;
        let mut hasher = Blake2s::new();
        loop {
            hasher.update(id);
            let nonce_bytes = nonce.to_le_bytes();
            hasher.update(nonce_bytes);
            if BinaryID::verify_difficulty(
                &mut hasher.finalize_reset().iter().rev(),
                K_DIFF_PRODUCED_BIT,
            ) {
                return Self {
                    bytes: id,
                    nonce: nonce_bytes,
                };
            }
            nonce += 1;
        }
    }

    pub fn verify_nonce(&self) -> bool {
        let mut hasher = Blake2s::new();
        hasher.update(self.bytes);
        hasher.update(self.nonce);
        BinaryID::verify_difficulty(&mut hasher.finalize().iter().rev(), K_DIFF_MIN_BIT)
    }

    pub(crate) fn verify_difficulty<'a, I>(bytes: &mut I, difficulty: usize) -> bool
    where
        I: Iterator<Item = &'a u8>,
    {
        bytes.next().map_or(false, |b| {
            if difficulty <= 8 {
                b.trailing_zeros() as usize >= difficulty
            } else if b != &0 {
                false
            } else {
                BinaryID::verify_difficulty(bytes, difficulty - 8)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{kbucket::BinaryID, peer::PeerNode};

    impl BinaryID {
        fn calculate_distance_native(&self, other: &BinaryID) -> Option<usize> {
            let a = u128::from_le_bytes(*self.as_binary());
            let b = u128::from_le_bytes(*other.as_binary());
            let xor = a ^ b;
            let ret = 128 - xor.leading_zeros() as usize;
            if ret == 0 {
                None
            } else {
                Some(ret - 1)
            }
        }
    }

    #[test]
    fn test_distance() {
        let n1 = PeerNode::from_address("192.168.0.1:666");
        let n2 = PeerNode::from_address("192.168.0.1:666");
        assert_eq!(n1.calculate_distance(&n2), None);
        assert_eq!(n1.id().calculate_distance_native(n2.id()), None);
        for i in 2..255 {
            let n_in = PeerNode::from_address(&format!("192.168.0.{}:666", i)[..]);
            assert_eq!(
                n1.calculate_distance(&n_in),
                n1.id().calculate_distance_native(n_in.id())
            );
        }
    }

    #[test]
    fn test_id_nonce() {
        let root = PeerNode::from_address("192.168.0.1:666");
        println!("Nonce is {:?}", root.id().nonce());
        assert!(root.id().verify_nonce());
    }

    #[test]
    fn test_difficulty() {
        let b: [u8; 2] = [0b11110000, 0];
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 0));
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 1));
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 2));
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 3));
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 4));
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 5));
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 6));
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 7));
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 8));
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 9));
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 10));
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 11));
        assert!(BinaryID::verify_difficulty(&mut b.iter().rev(), 12));
        assert!(!BinaryID::verify_difficulty(&mut b.iter().rev(), 13));
    }
}

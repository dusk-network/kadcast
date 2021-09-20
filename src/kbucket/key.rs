use crate::K_ID_LEN_BYTES;
use crate::K_NONCE_LEN;

pub type BinaryKey = [u8; K_ID_LEN_BYTES];
pub type BinaryNonce = [u8; K_NONCE_LEN];

use blake2::{Blake2s, Digest};

use crate::{K_DIFF_MIN_BIT, K_DIFF_PRODUCED_BIT};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct BinaryID {
    pub(crate) bytes: BinaryKey,
    pub(crate) nonce: BinaryNonce,
}

impl BinaryID {
    pub fn as_binary(&self) -> &BinaryKey {
        &self.bytes
    }
    pub fn nonce(&self) -> &BinaryNonce {
        &self.nonce
    }
    pub fn calculate_distance(&self, other: &Self) -> Option<usize> {
        let distance = BinaryID::xor(self.as_binary(), other.as_binary());
        let mut pos = 0;
        for (idx, bytes) in distance.iter().enumerate().rev() {
            if bytes == &0b0 {
                continue;
            };
            let mbsrank = 8 - bytes.leading_zeros() as usize;
            pos = mbsrank + (idx * 8);
            break;
        }
        if pos == 0 {
            None
        } else {
            Some(pos - 1)
        }
    }

    fn xor(one: &BinaryKey, two: &BinaryKey) -> Vec<u8> {
        one.iter()
            .zip(two.iter())
            .map(|(&x1, &x2)| x1 ^ x2)
            .collect()
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
    use crate::kbucket::BinaryID;

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

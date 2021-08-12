use blake2::{Blake2s, Digest};

use crate::{
    kbucket::{BinaryKey, BinaryNonce},
    K_DIFF_MIN_BIT, K_DIFF_PRODUCED_BIT,
};

pub(crate) fn xor(one: &BinaryKey, two: &BinaryKey) -> Vec<u8> {
    one.iter()
        .zip(two.iter())
        .map(|(&x1, &x2)| x1 ^ x2)
        .collect()
}

pub fn compute_nonce(id: &BinaryKey) -> BinaryNonce {
    if K_DIFF_PRODUCED_BIT < K_DIFF_MIN_BIT {
        panic!("PoW is less than minimum required, review your build config...")
    }
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

pub fn verify_nonce(id: &BinaryKey, nonce: &BinaryNonce) -> bool {
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
        } else if b != &0 {
            false
        } else {
            verify_difficulty(bytes, difficulty - 8)
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::utils::verify_difficulty;

    #[test]
    fn test_difficulty() {
        let b: [u8; 2] = [0b11110000, 0];
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

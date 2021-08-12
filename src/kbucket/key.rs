use crate::K_ID_LEN_BYTES;
use crate::K_NONCE_LEN;

pub type BinaryKey = [u8; K_ID_LEN_BYTES];
pub type BinaryNonce = [u8; K_NONCE_LEN];


pub trait BinaryID {
    fn as_binary(&self) -> &BinaryKey;
    fn nonce(&self) -> &BinaryNonce;
    fn calculate_distance(&self, other: &dyn BinaryID) -> Option<usize> {
        let distance = crate::utils::xor(self.as_binary(), other.as_binary());
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

    fn calculate_distance_native(&self, other: &dyn BinaryID) -> Option<usize> {
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

use crate::K_ID_LEN_BYTES;
use crate::K_NONCE_LEN;

pub type BinaryKey = [u8; K_ID_LEN_BYTES];
pub type BinaryNonce = [u8; K_NONCE_LEN];

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
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
}

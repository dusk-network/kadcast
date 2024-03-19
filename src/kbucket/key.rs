// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::io;

use crate::encoding::Marshallable;
use crate::K_ID_LEN_BYTES;
use crate::K_NONCE_LEN;

pub type BinaryKey = [u8; K_ID_LEN_BYTES];
pub type BinaryNonce = [u8; K_NONCE_LEN];

use blake2::{Blake2s256, Digest};

use crate::{K_DIFF_MIN_BIT, K_DIFF_PRODUCED_BIT};

use super::BucketHeight;

const _: () = assert!(
    (K_ID_LEN_BYTES * BucketHeight::BITS as usize) < BucketHeight::MAX as usize,
    "K_ID_LEN_BYTES must be lower than BucketHeight::MAX"
);

#[allow(clippy::assertions_on_constants)]
const _: () = assert!(
    K_DIFF_PRODUCED_BIT >= K_DIFF_MIN_BIT,
    "PoW is less than minimum required, review your build config..."
);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct BinaryID {
    bytes: BinaryKey,
    nonce: BinaryNonce,
}

impl Marshallable for BinaryKey {
    fn marshal_binary<W: std::io::Write>(
        &self,
        writer: &mut W,
    ) -> io::Result<()> {
        writer.write_all(self)?;
        Ok(())
    }

    fn unmarshal_binary<R: std::io::Read>(reader: &mut R) -> io::Result<Self>
    where
        Self: Sized,
    {
        let mut target = [0; K_ID_LEN_BYTES];
        reader.read_exact(&mut target)?;
        Ok(target)
    }
}

impl BinaryID {
    /// Returns the binary key associated with this `BinaryID`.
    pub fn as_binary(&self) -> &BinaryKey {
        &self.bytes
    }

    /// Returns the binary nonce associated with this `BinaryID`.
    pub fn nonce(&self) -> &BinaryNonce {
        &self.nonce
    }

    /// Calculates the 0-based Kadcast distance between two `BinaryID`s.
    ///
    /// Returns `None` if the two IDs are identical.
    pub fn calculate_distance(
        &self,
        other: &BinaryKey,
    ) -> Option<BucketHeight> {
        self.as_binary()
            .iter()
            .zip(other.iter())
            .map(|(&a, &b)| a ^ b)
            .enumerate()
            .rev()
            .find(|(_, b)| b != &0b0)
            .map(|(i, b)| (i as BucketHeight, b))
            .map(|(i, b)| BinaryID::msb(b).expect("to be Some") + (i << 3) - 1)
    }

    /// Returns the position of the most significant bit set in a byte.
    ///
    /// Returns `None` if no bit is set.
    const fn msb(n: u8) -> Option<u8> {
        match u8::BITS - n.leading_zeros() {
            0 => None,
            n => Some(n as u8),
        }
    }

    /// Creates a new `BinaryID` by combining a `BinaryKey` and a `BinaryNonce`.
    pub(crate) fn from_nonce(
        id: BinaryKey,
        nonce: BinaryNonce,
    ) -> io::Result<Self> {
        let ret = Self { bytes: id, nonce };
        if ret.verify_nonce() {
            Ok(ret)
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "Invalid Nonce"))
        }
    }

    /// Generates a new `BinaryID` using the given `BinaryKey`.
    ///
    /// This function generates a unique identifier by repeatedly hashing the
    /// provided `BinaryKey` with an incrementing nonce value until a
    /// difficulty threshold is met.
    pub(crate) fn generate(id: BinaryKey) -> Self {
        let mut nonce: u32 = 0;
        let mut hasher = Blake2s256::new();
        loop {
            hasher.update(id);
            let nonce_bytes = nonce.to_le_bytes();
            hasher.update(nonce_bytes);
            if BinaryID::verify_difficulty(
                &mut hasher.finalize_reset().iter(),
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

    /// Verifies that the nonce of the `BinaryID` meets the minimum difficulty
    /// requirements.
    pub fn verify_nonce(&self) -> bool {
        let mut hasher = Blake2s256::new();
        hasher.update(self.bytes);
        hasher.update(self.nonce);
        BinaryID::verify_difficulty(
            &mut hasher.finalize().iter(),
            K_DIFF_MIN_BIT,
        )
    }

    /// Verifies the difficulty of a binary value according to the specified
    /// criteria.
    pub(crate) fn verify_difficulty<'a, I>(
        bytes: &mut I,
        difficulty: usize,
    ) -> bool
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

    use super::*;
    use crate::kbucket::BucketHeight;
    use crate::peer::PeerNode;
    use crate::tests::Result;

    impl BinaryID {
        fn calculate_distance_native(
            &self,
            other: &BinaryID,
        ) -> Option<BucketHeight> {
            let a = u128::from_le_bytes(*self.as_binary());
            let b = u128::from_le_bytes(*other.as_binary());
            let xor = a ^ b;
            let ret = 128 - xor.leading_zeros() as usize;
            if ret == 0 {
                None
            } else {
                Some((ret - 1) as u8)
            }
        }
    }

    #[test]
    fn test_distance() -> Result<()> {
        let n1 = PeerNode::generate("192.168.0.1:666", 0)?;
        let n2 = PeerNode::generate("192.168.0.1:666", 0)?;
        assert_eq!(n1.calculate_distance(&n2), None);
        assert_eq!(n1.id().calculate_distance_native(n2.id()), None);
        for i in 2..255 {
            let n_in = PeerNode::generate(format!("192.168.0.{}:666", i), 0)?;
            assert_eq!(
                n1.calculate_distance(&n_in),
                n1.id().calculate_distance_native(n_in.id())
            );
        }
        Ok(())
    }

    #[test]
    fn test_id_nonce() -> Result<()> {
        let root = PeerNode::generate("192.168.0.1:666", 0)?;
        println!("Nonce is {:?}", root.id().nonce());
        assert!(root.id().verify_nonce());
        Ok(())
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

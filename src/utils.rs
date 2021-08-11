use crate::K_ID_LEN_BYTES;

pub fn xor(one: &[u8; K_ID_LEN_BYTES], two: &[u8; K_ID_LEN_BYTES]) -> Vec<u8> {
    one.iter()
        .zip(two.iter())
        .map(|(&x1, &x2)| x1 ^ x2)
        .collect()
}

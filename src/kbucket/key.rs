use std::convert::TryInto;

use crate::K_ID_LEN_BYTES;

pub trait BinaryID {
    fn as_binary(&self) -> &[u8; K_ID_LEN_BYTES];
    fn nonce(&self) -> u32;
    fn calculate_distance(&self, other: &dyn BinaryID) -> Option<usize> {
        let a = self.as_binary();
        let b = other.as_binary();
        // println!("a   : {:?}", &a);
        // println!("b   : {:?}", &b);
        let distance = crate::utils::xor(&a, &b);
        println!("xor : {:?}", &distance);
        let uno = u128::from_le_bytes(*a);
        let due = u128::from_le_bytes(*b);
        let distxor = uno^due;

        println!("a1  : {:0128b}", uno);
        println!("b1  : {:0128b}", due);
        println!("dis2 : {:0128b}", distxor);

        let distancebig = u128::from_le_bytes(distance.as_slice().try_into().unwrap());
        
        println!("dist : {:0128b}", distancebig);
        println!("zero : {}", distancebig.leading_zeros());
        let ret = 128-distancebig.leading_zeros() as usize;
            println!("distance is : {:?}", ret);
            println!("---------------------------------------------");
        if ret == 0 {
            None
        } else {
            Some(ret - 1)
        }
    }
    
}

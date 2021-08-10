use crate::K_ID_LEN;

pub trait BinaryID {

    fn as_binary(&self) -> [u8;K_ID_LEN];

    fn calculate_distance(&self, other: &dyn BinaryID) -> usize {
       let a = self.as_binary();
       let b = other.as_binary();
       //TODO: calculateDistance
       1
       
    }
    
}
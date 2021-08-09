pub trait BinaryID {

    fn as_binary(&self) -> bytes::Bytes;

    fn calculate_distance(&self, other: &dyn BinaryID) -> usize {
       let a = self.as_binary();
       let b = other.as_binary();
       //TODO: calculateDistance
       1
       
    }
    
}
use std::time::Instant;
use super::key::BinaryID;

pub struct Node<TKey:BinaryID , TValue> {
    id: TKey,
    value: TValue,
    last_used: Option<Instant>,
}

impl<TKey:BinaryID, TValue> Node<TKey, TValue> {

    pub fn new(id: TKey, value: TValue) -> Node<TKey, TValue>{
        Node{
            id,
            value,
            last_used:None
        }
    }

    pub fn calculate_distance(&self, other: &Node<TKey, TValue>) -> usize {
        self.id.calculate_distance(&other.id)
    }
    pub fn is_id_valid(&self) -> bool {
        true
    }

    pub fn id(&self) -> &TKey{
        &self.id
    }
    
}


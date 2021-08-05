use std::net::IpAddr;

struct Address {
    ip_addr: IpAddr,
    port: u16,
}

struct Node<ID, V> {
    id: ID,
    value: V,
}
struct Bucket<ID, V> {
    nodes: arrayvec::ArrayVec<Node<ID, V>, K_K>,
    // height: usize,
    last_boh_pos: usize,
}

enum InsertResult {
    Inserted,
    Pending,
    Rejected,
}

impl<ID, V> Bucket<ID, V> {
    fn insert(&mut self, node: Node<ID, V>) -> InsertResult {
        InsertResult::Pending
    }
}

//This should be derived from NodeID size (btw is the max amount of nodes in a network)
// const K_L: usize = 500;
// const K_K: NonZeroUsize = unsafe {NonZeroUsize::new_unchecked(100)};

//Max amount of nodes a bucket should contain
const K_K: usize = 20;

//Redundacy factor for lookup
const K_ALPHA: usize = 3;
//Redundacy factor for broadcast
const K_BETA: usize = 3;

#[cfg(test)]
mod tests {
    use arrayvec::ArrayVec;

    use crate::Bucket;
    use std::{array, collections::HashMap};
    #[test]
    fn it_works() {
        // let mut array = ArrayVec::<[Bucket<[u8;16]>; K_L]>::new();

        assert_eq!(2 + 2, 4);
    }
}

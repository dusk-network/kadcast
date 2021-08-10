
mod kbucket;
mod ktable;
mod peer;

//This should be derived from NodeID size (btw is the max amount of nodes in a network)
// const K_L: usize = 500;

// Max amount of nodes a bucket should contain
pub const K_K: usize = 20;
pub const K_ID_LEN:usize=16;
pub const K_NONCE_LEN:usize=4;

//Redundacy factor for lookup
const K_ALPHA: usize = 3;
//Redundacy factor for broadcast
const K_BETA: usize = 3;

const K_CHUNK_SIZE: usize = 1024;

#[cfg(test)]
mod tests {
    use crate::{kbucket::{BinaryID, InsertResult}, ktable::Tree, peer::{self}};

    
    #[test]
    fn test_id_nonce() {
        let root = peer::from_address(String::from("127.0.0.1:555"));
        let nonce = peer::compute_nonce(&root.id().as_binary());
        println!("Nonce is {}", nonce);
        assert!(peer::verify_nonce(&root.id().as_binary(), nonce));

    }

    #[test]
    fn it_works() {
        let root = peer::from_address(String::from("127.0.0.1:555"));
        let mut route_table = Tree::for_root(root);
        for i in 1..21 {
            let res = route_table.insert(peer::from_address(format!("192.168.0.{}:666", i)));
            assert!(if let InsertResult::Inserted = res {
                true
            } else {
                false
            });
        }
        let res = route_table.insert(peer::from_address(format!("192.168.0.{}:666", 100)));
        assert!(if let InsertResult::Rejected = res {
            true
        } else {
            false
        });

    }
}

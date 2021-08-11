mod kbucket;
mod ktable;
mod peer;
mod utils;

//This should be derived from NodeID size (btw is the max amount of nodes in a network)
// const K_L: usize = 500;

// Max amount of nodes a bucket should contain
pub const K_K: usize = 20;
pub const K_ID_LEN_BYTES: usize = 16;
pub const K_NONCE_LEN: usize = 4;
pub const K_DIFF_MIN_BIT: usize = 8;
pub const K_DIFF_PRODUCED_BIT: usize = 8;

//Redundacy factor for lookup
const K_ALPHA: usize = 3;
//Redundacy factor for broadcast
const K_BETA: usize = 3;

const K_CHUNK_SIZE: usize = 1024;

#[cfg(test)]
mod tests {
    use crate::{
        kbucket::{BinaryID, InsertResult},
        ktable::Tree,
        peer::{self},
    };

    #[test]
    fn test_id_nonce() {
        let root = peer::from_address(String::from("192.168.0.1:666"));
        let nonce = peer::compute_nonce(&root.id().as_binary());
        println!("Nonce is {:?}", nonce);
        assert!(peer::verify_nonce(&root.id().as_binary(), &nonce));
    }

    #[test]
    fn test_distance() {
        let n1 = peer::from_address(String::from("192.168.0.1:666"));
        let n2 = peer::from_address(String::from("192.168.0.1:666"));
        assert_eq!(n1.calculate_distance(&n2), None);
        assert_eq!(n1.id().calculate_distance_native(n2.id()), None);
        for i in 2..255 {
            let n_in = peer::from_address(format!("192.168.0.{}:666", i));
            assert_eq!(
                n1.calculate_distance(&n_in),
                n1.id().calculate_distance_native(n_in.id())
            );
        }
    }

    #[test]
    fn it_works() {
        let root = peer::from_address(String::from("192.168.0.1:666"));
        let mut route_table = Tree::for_root(root);
        for i in 2..255 {
            let res = route_table.insert(peer::from_address(format!("192.168.0.{}:666", i)));
            match res {
                InsertResult::Inserted | InsertResult::Pending(_) => assert!(true),
                _ => assert!(false),
            }
        }
        let res = route_table.insert(peer::from_address(String::from("192.168.0.1:666")));
        assert!(if let InsertResult::Invalid(_) = res {
            true
        } else {
            false
        });
    }
}

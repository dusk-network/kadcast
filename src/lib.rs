mod encoding;
pub mod kbucket;
mod peer;

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
    use std::time::Duration;

    use crate::{
        kbucket::{BinaryID, NodeInsertError},
        peer::PeerNode,
    };

    impl BinaryID {
        fn calculate_distance_native(&self, other: &BinaryID) -> Option<usize> {
            let a = u128::from_le_bytes(*self.as_binary());
            let b = u128::from_le_bytes(*other.as_binary());
            let xor = a ^ b;
            let ret = 128 - xor.leading_zeros() as usize;
            if ret == 0 {
                None
            } else {
                Some(ret - 1)
            }
        }
    }

    #[test]
    fn test_id_nonce() {
        let root = PeerNode::from_address("192.168.0.1:666");
        println!("Nonce is {:?}", root.id().nonce());
        assert!(root.id().verify_nonce());
    }

    #[test]
    fn test_distance() {
        let n1 = PeerNode::from_address("192.168.0.1:666");
        let n2 = PeerNode::from_address("192.168.0.1:666");
        assert_eq!(n1.calculate_distance(&n2), None);
        assert_eq!(n1.id().calculate_distance_native(n2.id()), None);
        for i in 2..255 {
            let n_in = PeerNode::from_address(&format!("192.168.0.{}:666", i)[..]);
            assert_eq!(
                n1.calculate_distance(&n_in),
                n1.id().calculate_distance_native(n_in.id())
            );
        }
    }

    #[test]
    fn it_works() {
        let root = PeerNode::from_address("192.168.0.1:666");
        let mut route_table = crate::kbucket::Tree::builder(root)
            .node_evict_after(Duration::from_millis(5000))
            .node_ttl(Duration::from_secs(60))
            .build();
        for i in 2..255 {
            let res =
                route_table.insert(PeerNode::from_address(&format!("192.168.0.{}:666", i)[..]));
            match res {
                Ok(_) => {}
                Err(error) => match error {
                    NodeInsertError::Invalid(_) => {
                        assert!(false)
                    }
                    _ => {}
                },
            }
        }
        let res = route_table
            .insert(PeerNode::from_address("192.168.0.1:666"))
            .expect_err("this should be an error");
        assert!(if let NodeInsertError::Invalid(_) = res {
            true
        } else {
            false
        });
    }
}

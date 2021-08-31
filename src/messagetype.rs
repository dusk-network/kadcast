// use crate::{K_CHUNK_SIZE, K_K, kbucket::{BinaryID, BinaryKey, BinaryNonce, Node}};

pub struct Ping {
    
    sender_id: BinaryKey,
    id_nonce: BinaryNonce,
}

// struct MessageHeader

pub enum KadcastMessageType<ID: BinaryID, V> {
    Ping(Ping),
    Pong {
        sender_id: BinaryKey,
        id_nonce: BinaryNonce,
    },
    FindNodes {
        sender_id: BinaryKey,
        target_id: BinaryKey,
    },
    Nodes {
        sender_id: BinaryKey,
        count: usize,
        node_tuples: [Node<ID, V>; K_K],
    }, //should we pass node[] as ref?
    Message {
        sender_id: BinaryKey,
        block_id: usize,
        chunk_id: usize,
        bcast_height: usize,
        chunk_data: [u8; K_CHUNK_SIZE],
    },
}
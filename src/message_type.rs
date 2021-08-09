
//not needed so far
enum KadcastMessageType<ID: BinaryID, V> {
    Ping {
        sender_id: ID,
        id_nonce: usize,
    },
    Pong {
        sender_id: ID,
        id_nonce: usize,
    },
    FindNodes {
        sender_id: ID,
        target_id: ID,
    },
    Nodes {
        sender_id: ID,
        count: usize,
        node_tuples: [Node<ID, V>; K_K],
    }, //should we pass node[] as ref?
    Message {
        sender_id: ID,
        block_id: usize,
        chunk_id: usize,
        bcast_height: usize,
        chunk_data: [u8; K_CHUNK_SIZE],
    },
}
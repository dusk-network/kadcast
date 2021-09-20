use std::{
    error::Error,
    io::{BufReader, BufWriter, Read, Write},
};

pub mod error;
mod header;
pub mod message;
pub(crate) mod payload;

pub trait Marshallable {
    fn marshal_binary<W: Write>(&self, writer: &mut BufWriter<W>) -> Result<(), Box<dyn Error>>;
    fn unmarshal_binary<R: Read>(reader: &mut BufReader<R>) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized;
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, BufWriter, Cursor, Read, Seek};

    use crate::{
        encoding::{
            message::KadcastMessage,
            payload::{BroadcastPayload, NodePayload},
        },
        peer::PeerNode,
    };

    use super::Marshallable;

    #[test]
    fn encode_ping() {
        let peer = PeerNode::from_address("192.168.0.1:666");
        let a = KadcastMessage::Ping(peer.as_header());
        test_kadkast_marshal(a);
    }
    #[test]
    fn encode_pong() {
        let peer = PeerNode::from_address("192.168.0.1:666");
        let a = KadcastMessage::Pong(peer.as_header());
        test_kadkast_marshal(a);
        assert_eq!(1, 1);
    }

    #[test]
    fn encode_find_nodes() {
        let peer = PeerNode::from_address("192.168.0.1:666");
        let nodes = vec![
            PeerNode::from_address("192.168.1.1:666"),
            PeerNode::from_address("[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:666"),
        ]
        .iter()
        .map(|f| f.as_peer_info())
        .collect();
        let a = KadcastMessage::FindNodes(peer.as_header(), NodePayload { peers: nodes });
        test_kadkast_marshal(a);
        assert_eq!(1, 1);
    }

    #[test]
    fn encode_nodes() {
        let peer = PeerNode::from_address("192.168.0.1:666");
        let nodes = vec![
            PeerNode::from_address("192.168.1.1:666"),
            PeerNode::from_address("[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:666"),
        ]
        .iter()
        .map(|f| f.as_peer_info())
        .collect();
        let a = KadcastMessage::Nodes(peer.as_header(), NodePayload { peers: nodes });
        test_kadkast_marshal(a);
        assert_eq!(1, 1);
    }

    #[test]
    fn encode_broadcast() {
        let peer = PeerNode::from_address("192.168.0.1:666");
        let a = KadcastMessage::Broadcast(
            peer.as_header(),
            BroadcastPayload {
                height: 10,
                gossip_frame: vec![3, 5, 6, 7],
            },
        );
        test_kadkast_marshal(a);
        assert_eq!(1, 1);
    }

    fn test_kadkast_marshal(messge: KadcastMessage) {
        println!("orig: {:?}", messge);
        let mut c = Cursor::new(Vec::new());
        let mut writer = BufWriter::new(c);
        messge.marshal_binary(&mut writer).unwrap();
        c = writer.into_inner().unwrap();
        let mut bytes = vec![];
        c.rewind().unwrap();
        c.read_to_end(&mut bytes).unwrap();
        c.rewind().unwrap();
        println!("bytes: {:?}", bytes);
        // c.rewind().unwrap();
        let mut reader = BufReader::new(c);
        let deser = KadcastMessage::unmarshal_binary(&mut reader).unwrap();

        println!("dese: {:?}", deser);
        assert_eq!(messge, deser);
    }
}

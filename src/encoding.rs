// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::io::{self, Read, Write};

mod header;
pub mod message;
pub(crate) mod payload;

pub trait Marshallable {
    fn marshal_binary<W: Write>(&self, writer: &mut W) -> io::Result<()>;
    fn unmarshal_binary<R: Read>(reader: &mut R) -> io::Result<Self>
    where
        Self: Sized;
}

#[cfg(test)]
mod tests {

    use std::io::{BufReader, BufWriter, Cursor, Read, Seek};

    use super::Marshallable;
    use crate::encoding::message::Message;
    use crate::encoding::payload::{BroadcastPayload, NodePayload};
    use crate::peer::PeerNode;
    use crate::tests::Result;

    #[test]
    fn test_encode_ping() -> Result<()> {
        let peer = PeerNode::generate("192.168.0.1:666")?;
        let a = Message::Ping(peer.as_header());
        test_kadkast_marshal(a)
    }
    #[test]
    fn test_encode_pong() -> Result<()> {
        let peer = PeerNode::generate("192.168.0.1:666")?;
        let a = Message::Pong(peer.as_header());
        test_kadkast_marshal(a)
    }

    #[test]
    fn test_encode_find_nodes() -> Result<()> {
        let peer = PeerNode::generate("192.168.0.1:666")?;
        let target = *PeerNode::generate("192.168.1.1:666")?.id().as_binary();
        let a = Message::FindNodes(peer.as_header(), target);
        test_kadkast_marshal(a)
    }

    #[test]
    fn test_encode_nodes() -> Result<()> {
        let peer = PeerNode::generate("192.168.0.1:666")?;
        let nodes = vec![
            PeerNode::generate("192.168.1.1:666")?,
            PeerNode::generate(
                "[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:666",
            )?,
        ]
        .iter()
        .map(|f| f.as_peer_info())
        .collect();
        let a = Message::Nodes(peer.as_header(), NodePayload { peers: nodes });
        test_kadkast_marshal(a)
    }

    #[test]
    fn test_encode_empty_nodes() -> Result<()> {
        let peer = PeerNode::generate("192.168.0.1:666")?;
        let a = Message::Nodes(peer.as_header(), NodePayload { peers: vec![] });
        test_kadkast_marshal(a)
    }
    #[test]
    fn test_encode_broadcast() -> Result<()> {
        let peer = PeerNode::generate("192.168.0.1:666")?;
        let a = Message::Broadcast(
            peer.as_header(),
            BroadcastPayload {
                height: 10,
                gossip_frame: vec![3, 5, 6, 7],
            },
        );
        test_kadkast_marshal(a)
    }

    fn test_kadkast_marshal(messge: Message) -> Result<()> {
        println!("orig: {:?}", messge);
        let mut c = Cursor::new(Vec::new());
        let mut writer = BufWriter::new(c);
        messge.marshal_binary(&mut writer)?;
        c = writer.into_inner()?;
        let mut bytes = vec![];
        c.seek(std::io::SeekFrom::Start(0))?;
        c.read_to_end(&mut bytes)?;
        c.seek(std::io::SeekFrom::Start(0))?;
        println!("bytes: {:?}", bytes);
        println!("byhex: {:02X?}", bytes);
        let mut reader = BufReader::new(c);
        let deser = Message::unmarshal_binary(&mut reader)?;

        println!("dese: {:?}", deser);
        assert_eq!(messge, deser);
        Ok(())
    }
}

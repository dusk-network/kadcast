use std::{
    error::Error,
    io::{BufReader, BufWriter, Read, Write},
};

use crate::encoding::error::EncodingError;

use super::payload::{BroadcastPayload, NodePayload};
pub use super::{header::Header, Marshallable};

// PingMsg wire Ping message id.
const ID_MSG_PING: u8 = 0;

// PongMsg wire Pong message id.
const ID_MSG_PONG: u8 = 1;

// FindNodesMsg wire FindNodes message id.
const ID_MSG_FIND_NODES: u8 = 2;

// NodesMsg wire Nodes message id.
const ID_MSG_NODES: u8 = 3;

// BroadcastMsg Message propagation type.
const ID_MSG_BROADCAST: u8 = 10;

#[derive(Debug,PartialEq)]
pub enum KadcastMessage {
    Ping(Header),
    Pong(Header),
    FindNodes(Header, NodePayload),
    Nodes(Header, NodePayload), //should we pass node[] as ref?
    Broadcast(Header, BroadcastPayload),
}

impl KadcastMessage {
    fn type_byte(&self) -> u8 {
        match self {
            KadcastMessage::Ping(_) => ID_MSG_PING,
            KadcastMessage::Pong(_) => ID_MSG_PONG,
            KadcastMessage::FindNodes(_, _) => ID_MSG_FIND_NODES,
            KadcastMessage::Nodes(_, _) => ID_MSG_NODES,
            KadcastMessage::Broadcast(_, _) => ID_MSG_BROADCAST,
        }
    }
}

impl Marshallable for KadcastMessage {
    fn marshal_binary<W: Write>(&self, writer: &mut BufWriter<W>) -> Result<(), Box<dyn Error>> {
        writer.write_all(&[self.type_byte()])?;
        match self {
            KadcastMessage::Ping(h) | KadcastMessage::Pong(h) => h.marshal_binary(writer)?,
            KadcastMessage::FindNodes(h, p) | KadcastMessage::Nodes(h, p) => {
                h.marshal_binary(writer)?;
                p.marshal_binary(writer)?;
            }
            KadcastMessage::Broadcast(h, p) => {
                h.marshal_binary(writer)?;
                p.marshal_binary(writer)?;
            }
        };
        Ok(writer.flush()?)
    }

    fn unmarshal_binary<W: Read>(
        reader: &mut BufReader<W>,
    ) -> Result<KadcastMessage, Box<dyn Error>> {
        let mut message_type = [0; 1];
        reader.read_exact(&mut message_type)?;
        let header = Header::unmarshal_binary(reader)?;
        match message_type[0] {
            ID_MSG_PING => Ok(KadcastMessage::Ping(header)),
            ID_MSG_PONG => Ok(KadcastMessage::Pong(header)),
            ID_MSG_FIND_NODES => {
                let payload = NodePayload::unmarshal_binary(reader)?;
                Ok(KadcastMessage::FindNodes(header, payload))
            }
            ID_MSG_NODES => {
                let payload = NodePayload::unmarshal_binary(reader)?;
                Ok(KadcastMessage::Nodes(header, payload))
            }
            ID_MSG_BROADCAST => {
                let payload = BroadcastPayload::unmarshal_binary(reader)?;
                Ok(KadcastMessage::Broadcast(header, payload))
            }
            unknown => Err(Box::new(EncodingError::new(&format!(
                "Invalid message type: '{}'",
                unknown
            )))),
        }
    }
}

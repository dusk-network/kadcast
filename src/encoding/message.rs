// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::io::{self, Error, ErrorKind, Read, Write};

pub(crate) use super::payload::{BroadcastPayload, NodePayload};
pub use super::{header::Header, Marshallable};
use crate::kbucket::BinaryKey;

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

#[derive(Debug, PartialEq)]
pub(crate) enum Message {
    Ping(Header),
    Pong(Header),
    FindNodes(Header, BinaryKey),
    Nodes(Header, NodePayload), //should we pass node[] as ref?
    Broadcast(Header, BroadcastPayload),
}

impl Message {
    pub(crate) fn type_byte(&self) -> u8 {
        match self {
            Message::Ping(_) => ID_MSG_PING,
            Message::Pong(_) => ID_MSG_PONG,
            Message::FindNodes(_, _) => ID_MSG_FIND_NODES,
            Message::Nodes(_, _) => ID_MSG_NODES,
            Message::Broadcast(_, _) => ID_MSG_BROADCAST,
        }
    }

    pub(crate) fn header(&self) -> &Header {
        match self {
            Message::Ping(header) => header,
            Message::Pong(header) => header,
            Message::FindNodes(header, _) => header,
            Message::Nodes(header, _) => header,
            Message::Broadcast(header, _) => header,
        }
    }

    pub(crate) fn bytes(&self) -> io::Result<Vec<u8>> {
        let mut bytes = vec![];
        self.marshal_binary(&mut bytes)?;
        Ok(bytes)
    }
}

impl Marshallable for Message {
    fn marshal_binary<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&[self.type_byte()])?;
        match self {
            Message::Ping(header) | Message::Pong(header) => {
                header.marshal_binary(writer)?
            }
            Message::FindNodes(header, target) => {
                header.marshal_binary(writer)?;
                target.marshal_binary(writer)?;
            }
            Message::Nodes(header, node_payload) => {
                header.marshal_binary(writer)?;
                node_payload.marshal_binary(writer)?;
            }
            Message::Broadcast(header, broadcast_payload) => {
                header.marshal_binary(writer)?;
                broadcast_payload.marshal_binary(writer)?;
            }
        };
        writer.flush()?;
        Ok(())
    }

    fn unmarshal_binary<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut message_type = [0; 1];
        reader.read_exact(&mut message_type)?;
        let header = Header::unmarshal_binary(reader)?;
        match message_type[0] {
            ID_MSG_PING => Ok(Message::Ping(header)),
            ID_MSG_PONG => Ok(Message::Pong(header)),
            ID_MSG_FIND_NODES => {
                let target = BinaryKey::unmarshal_binary(reader)?;
                Ok(Message::FindNodes(header, target))
            }
            ID_MSG_NODES => {
                let payload = NodePayload::unmarshal_binary(reader)?;
                Ok(Message::Nodes(header, payload))
            }
            ID_MSG_BROADCAST => {
                let payload = BroadcastPayload::unmarshal_binary(reader)?;
                Ok(Message::Broadcast(header, payload))
            }
            unknown => Err(Error::new(
                ErrorKind::Other,
                format!("Invalid message type: '{}'", unknown),
            )),
        }
    }
}

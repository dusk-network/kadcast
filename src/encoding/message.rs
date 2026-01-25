// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::io::{self, Error, Read, Write};

use semver::Version;

pub(crate) use super::payload::{BroadcastPayload, NodePayload};
pub use super::{Marshallable, header::Header};
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
    Ping(Header, Version),
    Pong(Header, Version),
    FindNodes(Header, Version, BinaryKey),
    Nodes(Header, Version, NodePayload), //should we pass node[] as ref?
    Broadcast(Header, BroadcastPayload, [u8; 32]),
}

impl Message {
    pub fn broadcast(header: Header, payload: BroadcastPayload) -> Self {
        Self::Broadcast(header, payload, [0; 32])
    }

    pub(crate) fn type_byte(&self) -> u8 {
        match self {
            Message::Ping(..) => ID_MSG_PING,
            Message::Pong(..) => ID_MSG_PONG,
            Message::FindNodes(..) => ID_MSG_FIND_NODES,
            Message::Nodes(..) => ID_MSG_NODES,
            Message::Broadcast(..) => ID_MSG_BROADCAST,
        }
    }

    pub(crate) fn header(&self) -> &Header {
        match self {
            Message::Ping(header, ..) => header,
            Message::Pong(header, ..) => header,
            Message::FindNodes(header, ..) => header,
            Message::Nodes(header, ..) => header,
            Message::Broadcast(header, ..) => header,
        }
    }

    pub(crate) fn bytes(&self) -> io::Result<Vec<u8>> {
        let mut bytes = vec![];
        self.marshal_binary(&mut bytes)?;
        Ok(bytes)
    }

    pub(crate) fn version(&self) -> Option<&Version> {
        match self {
            Message::Ping(_, version) => Some(version),
            Message::Pong(_, version) => Some(version),
            Message::FindNodes(_, version, _) => Some(version),
            Message::Nodes(_, version, _) => Some(version),

            _ => None,
        }
    }
}

impl Marshallable for semver::Version {
    fn marshal_binary<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&[self.major as u8])?;
        writer.write_all(&(self.minor as u16).to_le_bytes())?;
        writer.write_all(&(self.patch as u16).to_le_bytes())?;
        Ok(())
    }

    fn unmarshal_binary<R: Read>(reader: &mut R) -> io::Result<Self>
    where
        Self: Sized,
    {
        let mut maj = [0; 1];
        reader.read_exact(&mut maj)?;

        let mut min = [0; 2];
        reader.read_exact(&mut min)?;

        let mut patch = [0; 2];
        reader.read_exact(&mut patch)?;

        Ok(semver::Version::new(
            maj[0] as u64,
            u16::from_le_bytes(min) as u64,
            u16::from_le_bytes(patch) as u64,
        ))
    }
}

impl Marshallable for Message {
    fn marshal_binary<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&[self.type_byte()])?;
        self.header().marshal_binary(writer)?;
        match self {
            Message::Ping(_, version) | Message::Pong(_, version) => {
                version.marshal_binary(writer)?;
            }
            Message::FindNodes(_, version, target) => {
                version.marshal_binary(writer)?;
                target.marshal_binary(writer)?;
            }
            Message::Nodes(_, version, node_payload) => {
                version.marshal_binary(writer)?;
                node_payload.marshal_binary(writer)?;
            }
            Message::Broadcast(_, broadcast_payload, ..) => {
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
            ID_MSG_PING => {
                let version = Version::unmarshal_binary(reader)?;
                Ok(Message::Ping(header, version))
            }
            ID_MSG_PONG => {
                let version = Version::unmarshal_binary(reader)?;
                Ok(Message::Pong(header, version))
            }
            ID_MSG_FIND_NODES => {
                let version = Version::unmarshal_binary(reader)?;
                let target = BinaryKey::unmarshal_binary(reader)?;
                Ok(Message::FindNodes(header, version, target))
            }
            ID_MSG_NODES => {
                let version = Version::unmarshal_binary(reader)?;
                let payload = NodePayload::unmarshal_binary(reader)?;
                Ok(Message::Nodes(header, version, payload))
            }
            ID_MSG_BROADCAST => {
                let payload = BroadcastPayload::unmarshal_binary(reader)?;
                Ok(Message::broadcast(header, payload))
            }
            unknown => Err(Error::other(format!(
                "Invalid message type: '{}'",
                unknown
            ))),
        }
    }
}

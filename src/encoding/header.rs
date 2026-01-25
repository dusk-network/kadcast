// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::io::{self, Error, Read, Write};

use super::Marshallable;
use crate::{K_ID_LEN_BYTES, K_NONCE_LEN, kbucket::BinaryID};

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Header {
    pub(crate) binary_id: BinaryID,
    pub(crate) sender_port: u16,
    pub(crate) network_id: u8,
    pub(crate) reserved: [u8; 2],
}

impl Header {
    pub fn binary_id(&self) -> &BinaryID {
        &self.binary_id
    }
}

impl Marshallable for Header {
    fn marshal_binary<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        if !self.binary_id.verify_nonce() {
            return Err(Error::other("Invalid Nonce"));
        }
        writer.write_all(self.binary_id.as_binary())?;
        writer.write_all(self.binary_id.nonce())?;
        writer.write_all(&self.sender_port.to_le_bytes())?;
        writer.write_all(&[self.network_id])?;
        writer.write_all(&self.reserved)?;
        Ok(())
    }

    fn unmarshal_binary<R: Read>(reader: &mut R) -> io::Result<Self>
    where
        Self: Sized,
    {
        let mut id = [0; K_ID_LEN_BYTES];
        reader.read_exact(&mut id)?;
        let mut nonce = [0; K_NONCE_LEN];
        reader.read_exact(&mut nonce)?;

        let binary_id = BinaryID::from_nonce(id, nonce)?;

        let mut port_buffer = [0; 2];
        reader.read_exact(&mut port_buffer)?;
        let sender_port = u16::from_le_bytes(port_buffer);

        let mut network_id = [0; 1];
        reader.read_exact(&mut network_id)?;
        let network_id = network_id[0];

        let mut reserved = [0; 2];
        reader.read_exact(&mut reserved)?;

        Ok(Header {
            binary_id,
            sender_port,
            network_id,
            reserved,
        })
    }
}

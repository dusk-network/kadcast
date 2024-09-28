// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::io::{self, Read, Write};

use crate::encoding::Marshallable;

#[derive(Debug, PartialEq)]
pub(crate) struct BroadcastPayload {
    pub(crate) height: u8,
    pub(crate) gossip_frame: Vec<u8>,
    pub(crate) ray_id: Vec<u8>,
}

impl Marshallable for BroadcastPayload {
    fn marshal_binary<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&[self.height])?;
        let len = self.gossip_frame.len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(&self.gossip_frame)?;
        Ok(())
    }
    fn unmarshal_binary<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut height_buf = [0; 1];
        reader.read_exact(&mut height_buf)?;
        let mut gossip_length_buf = [0; 4];
        reader.read_exact(&mut gossip_length_buf)?;
        let gossip_length = u32::from_le_bytes(gossip_length_buf);
        let mut gossip_frame = vec![0; gossip_length as usize];
        reader.read_exact(&mut gossip_frame)?;
        Ok(BroadcastPayload {
            height: height_buf[0],
            gossip_frame,
            ray_id: vec![], /* We don't serialize the ray_id because is up to
                             * the decoder. */
        })
    }
}

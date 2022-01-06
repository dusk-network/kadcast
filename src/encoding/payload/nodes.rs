// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::convert::TryInto;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use crate::{encoding::Marshallable, kbucket::BinaryKey, K_ID_LEN_BYTES};
#[derive(Debug, PartialEq)]
pub(crate) struct NodePayload {
    pub(crate) peers: Vec<PeerEncodedInfo>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct PeerEncodedInfo {
    pub(crate) ip: IpInfo,
    pub(crate) port: u16,
    pub(crate) id: BinaryKey,
}
#[derive(Debug, PartialEq)]
pub enum IpInfo {
    IPv4([u8; 4]),
    IPv6([u8; 16]),
}

impl PeerEncodedInfo {
    pub(crate) fn to_socket_address(&self) -> SocketAddr {
        match self.ip {
            IpInfo::IPv4(bytes) => SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::from(bytes),
                self.port,
            )),
            IpInfo::IPv6(bytes) => SocketAddr::V6(SocketAddrV6::new(
                Ipv6Addr::from(bytes),
                self.port,
                0,
                0,
            )),
        }
    }
}

impl Marshallable for PeerEncodedInfo {
    fn marshal_binary<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match &self.ip {
            IpInfo::IPv6(bytes) => {
                writer.write_all(&[0u8])?;
                writer.write_all(bytes)?;
            }
            IpInfo::IPv4(bytes) => {
                writer.write_all(bytes)?;
            }
        }
        writer.write_all(&self.port.to_le_bytes())?;
        writer.write_all(&self.id)?;
        Ok(())
    }

    fn unmarshal_binary<R: Read>(reader: &mut R) -> io::Result<Self> {
        let concat_u8 = |first: &[u8], second: &[u8]| -> Vec<u8> {
            [first, second].concat()
        };
        let mut ipv4 = [0; 4];
        let ip: IpInfo;
        reader.read_exact(&mut ipv4)?;
        if ipv4[0] != 0u8 {
            ip = IpInfo::IPv4(ipv4)
        } else {
            let mut ipv6 = [0u8; 13];
            reader.read_exact(&mut ipv6)?;
            let ipv6_bytes: [u8; 16] = concat_u8(&ipv4[1..], &ipv6[..])
                .as_slice()
                .try_into()
                .expect("Wrong length");

            ip = IpInfo::IPv6(ipv6_bytes);
        }
        let mut port = [0; 2];
        reader.read_exact(&mut port)?;
        let port = u16::from_le_bytes(port);
        let mut id = [0; K_ID_LEN_BYTES];
        reader.read_exact(&mut id)?;
        Ok(PeerEncodedInfo { ip, port, id })
    }
}
impl Marshallable for NodePayload {
    fn marshal_binary<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let len = self.peers.len() as u16;
        writer.write_all(&len.to_le_bytes())?;
        for peer in &self.peers {
            peer.marshal_binary(writer)?
        }
        Ok(())
    }
    fn unmarshal_binary<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut peers: Vec<PeerEncodedInfo> = vec![];
        let mut len = [0; 2];
        reader.read_exact(&mut len)?;
        for _ in 0..u16::from_le_bytes(len) {
            peers.push(PeerEncodedInfo::unmarshal_binary(reader)?)
        }
        Ok(NodePayload { peers })
    }
}

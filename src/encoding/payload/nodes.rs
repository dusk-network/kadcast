use std::{
    convert::TryInto,
    error::Error,
    io::{BufReader, BufWriter, Read, Write},
};

use crate::{encoding::error::EncodingError, kbucket::BinaryKey, K_ID_LEN_BYTES};

use super::Marshallable;

pub struct NodePayload {
    peers: Vec<PeerInfo>,
}

struct PeerInfo {
    ip: IpInfo,
    port: u16,
    id: BinaryKey,
}

enum IpInfo {
    IPv4([u8; 4]),
    IPv6([u8; 16]),
}

impl Marshallable for PeerInfo {
    fn marshal_binary<W: Write>(&self, writer: &mut BufWriter<W>) -> Result<(), Box<dyn Error>> {
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

    fn unmarshal_binary<R: Read>(reader: &mut BufReader<R>) -> Result<PeerInfo, Box<dyn Error>> {
        let concat_u8 = |first: &[u8], second: &[u8]| -> Vec<u8> { [first, second].concat() };
        let mut ipv4 = [0; 4];
        let ip: IpInfo;
        reader.read_exact(&mut ipv4)?;
        if ipv4[0] == 0u8 {
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
        Ok(PeerInfo { ip, port, id })
    }
}
impl Marshallable for NodePayload {
    fn marshal_binary<W: Write>(&self, writer: &mut BufWriter<W>) -> Result<(), Box<dyn Error>> {
        let len = self.peers.len() as u16;
        if len == 0 {
            return Err(Box::new(EncodingError::new("Invalid peers count")));
        }
        writer.write_all(&len.to_le_bytes())?;
        for peer in &self.peers {
            peer.marshal_binary(writer)?
        }
        Ok(())
    }
    fn unmarshal_binary<R: Read>(reader: &mut BufReader<R>) -> Result<NodePayload, Box<dyn Error>> {
        let mut peers: Vec<PeerInfo> = vec![];
        let mut len = [0; 2];
        reader.read_exact(&mut len)?;
        let len = u16::from_le_bytes(len);
        if len == 0 {
            return Err(Box::new(EncodingError::new("Invalid peers count")));
        }
        for _ in 0..len {
            peers.push(PeerInfo::unmarshal_binary(reader)?)
        }
        Ok(NodePayload { peers })
    }
}

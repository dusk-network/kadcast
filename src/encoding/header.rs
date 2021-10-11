use std::{
    error::Error,
    io::{Read, Write},
};

use crate::{kbucket::BinaryID, K_ID_LEN_BYTES, K_NONCE_LEN};

use super::{error::EncodingError, Marshallable};
#[derive(Debug, PartialEq)]
pub struct Header {
    pub(crate) binary_id: BinaryID,
    pub(crate) sender_port: u16,
    pub(crate) reserved: [u8; 2],
}

impl Marshallable for Header {
    fn marshal_binary<W: Write>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        if !self.binary_id.verify_nonce() {
            return Err(Box::new(EncodingError::new("Invalid Nonce")));
        }
        writer.write_all(self.binary_id.as_binary())?;
        writer.write_all(self.binary_id.nonce())?;
        writer.write_all(&self.sender_port.to_le_bytes())?;
        writer.write_all(&self.reserved)?;
        Ok(())
    }

    fn unmarshal_binary<R: Read>(reader: &mut R) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized,
    {
        let mut id = [0; K_ID_LEN_BYTES];
        reader.read_exact(&mut id)?;
        let mut nonce = [0; K_NONCE_LEN];
        reader.read_exact(&mut nonce)?;
        let binary_id = BinaryID::from_nonce(id, nonce);
        if !binary_id.verify_nonce() {
            return Err(Box::new(EncodingError::new("Invalid Nonce")));
        }

        let mut port_buffer = [0; 2];
        reader.read_exact(&mut port_buffer)?;
        let port = u16::from_le_bytes(port_buffer);
        let mut reserved = [0; 2];
        reader.read_exact(&mut reserved)?;
        Ok(Header {
            binary_id,
            sender_port: port,
            reserved,
        })
    }
}

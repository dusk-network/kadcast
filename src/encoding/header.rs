use std::{
    error::Error,
    io::{BufWriter, Read, Write},
};

use crate::{
    kbucket::{BinaryKey, BinaryNonce},
    utils, K_ID_LEN_BYTES, K_NONCE_LEN,
};

use super::{error::EncodingError, Marshallable};

pub struct Header {
    id: BinaryKey,
    nonce: BinaryNonce, //we should changeit to u32 according to golang reference impl?
    port: u16,          //why we have the port here?????
    reserved: [u8; 2],
}

impl Marshallable for Header {
    fn marshal_binary<W: Write>(&self, writer: &mut BufWriter<W>) -> Result<(), Box<dyn Error>> {
        if !utils::verify_nonce(&self.id, &self.nonce) {
            return Err(Box::new(EncodingError::new("Invalid Nonce")));
        }
        writer.write_all(&self.id)?;
        writer.write_all(&self.nonce)?;
        writer.write_all(&self.port.to_le_bytes())?;
        writer.write_all(&self.reserved)?;
        Ok(())
    }

    fn unmarshal_binary<R: std::io::Read>(
        reader: &mut std::io::BufReader<R>,
    ) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized,
    {
        let mut id = [0; K_ID_LEN_BYTES];
        reader.read_exact(&mut id)?;
        let mut nonce = [0; K_NONCE_LEN];
        reader.read_exact(&mut nonce)?;
        if !utils::verify_nonce(&id, &nonce) {
            return Err(Box::new(EncodingError::new("Invalid Nonce")));
        }

        let mut port_buffer = [0; 2];
        reader.read_exact(&mut port_buffer)?;
        let port = u16::from_le_bytes(port_buffer);
        let mut reserved = [0; 2];
        reader.read_exact(&mut reserved)?;
        Ok(Header {
            id,
            nonce,
            port,
            reserved,
        })
    }
}

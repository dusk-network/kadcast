use std::{
    error::Error,
    io::{BufWriter, Write},
};

use crate::{
    kbucket::{BinaryKey, BinaryNonce},
    utils,
};

use super::{error::EncodingError, payload::Marshallable};

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

    fn unmarshal_binary<R: std::io::Read>(_reader: &mut std::io::BufReader<R>) -> Result<Self, Box<dyn Error>>
    where Self: Sized {
        todo!()
    }
}

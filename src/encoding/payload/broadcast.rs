use std::{error::Error, io::{BufReader, BufWriter, Read, Write}};

use super::Marshallable;
pub struct BroadcastPayload {
    height: u8,
    gossip_frame: Box<[u8]>, //this is the result of any internal protocol message serialization
}

impl Marshallable for BroadcastPayload {
    fn marshal_binary<W: Write>(&self, writer: &mut BufWriter<W>) -> Result<(), Box<dyn Error>> {
        writer.write_all(&[self.height])?;
        let len = self.gossip_frame.len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(&self.gossip_frame)?;
        Ok(())
    }
    fn unmarshal_binary<R: Read>(
        _reader: &mut BufReader<R>,
    ) -> Result<BroadcastPayload, Box<dyn Error>> {
        todo!()
    }
}

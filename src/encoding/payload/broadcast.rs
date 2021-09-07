use std::{
    error::Error,
    io::{BufReader, BufWriter, Read, Write},
};

use crate::encoding::Marshallable;
#[derive(Debug, PartialEq)]
pub(crate) struct BroadcastPayload {
    pub(crate) height: u8,            //HELP: what this height refer to?
    pub(crate) gossip_frame: Vec<u8>, //Is this the result of any internal protocol message serialization?
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
        reader: &mut BufReader<R>,
    ) -> Result<BroadcastPayload, Box<dyn Error>> {
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
        })
    }
}

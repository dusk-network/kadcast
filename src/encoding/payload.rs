pub(super) mod broadcast;
pub(super) mod nodes;
pub(super) use crate::encoding::payload::broadcast::BroadcastPayload;
pub(super) use crate::encoding::payload::nodes::NodePayload;
use std::{error::Error, io::{BufReader, BufWriter, Read, Write}};

pub trait Marshallable {
    fn marshal_binary<W: Write>(&self, writer: &mut BufWriter<W>) -> Result<(), Box<dyn Error>>;
    fn unmarshal_binary<R: Read>(reader: &mut BufReader<R>) -> Result<Self, Box<dyn Error>> where Self: Sized ;
}
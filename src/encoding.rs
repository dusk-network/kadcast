use std::{
    error::Error,
    io::{BufReader, BufWriter, Read, Write},
};

pub mod error;
mod header;
pub mod message;
mod payload;

pub trait Marshallable {
    fn marshal_binary<W: Write>(&self, writer: &mut BufWriter<W>) -> Result<(), Box<dyn Error>>;
    fn unmarshal_binary<R: Read>(reader: &mut BufReader<R>) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized;
}

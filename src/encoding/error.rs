use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub(super) struct EncodingError {
    details: String,
}

impl EncodingError {
    pub(super) fn new(msg: &str) -> EncodingError {
        EncodingError {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for EncodingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for EncodingError {
    fn description(&self) -> &str {
        &self.details
    }
}

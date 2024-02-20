use derive_error::Error;
use std::{
    fmt::{Display, Formatter},
    num::TryFromIntError,
};

#[derive(Error, Debug)]
pub enum TransportError {
    Io(std::io::Error),
    Cid(cid::Error),
    AdHoc(AdHocError),
    Scale(parity_scale_codec::Error),
    TimedOut,
    IntegerValueOutOfBounds(TryFromIntError),
}

pub type Result<T> = std::result::Result<T, TransportError>;

pub fn adhoc(msg: &str) -> TransportError {
    TransportError::AdHoc(AdHocError {
        message: msg.to_owned(),
    })
}
pub fn adhoc_err(msg: &str) -> Result<()> {
    Err(adhoc(msg))
}

#[derive(Debug)]
pub struct AdHocError {
    pub message: String,
}

impl Display for AdHocError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AdHocError {}

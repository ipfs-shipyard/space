use derive_error::Error;
use std::io;
use std::num::ParseIntError;

#[derive(Debug, Error)]
pub enum Error {
    Io(io::Error),
    ParseInt(ParseIntError),
}

pub type Result<T> = std::result::Result<T, Error>;

use derive_error::Error;

#[derive(Debug, Error)]
pub enum Error {
    Cid(cid::Error),
    EmptyCidList,
}

pub type Result<T> = std::result::Result<T, Error>;

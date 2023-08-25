use derive_error::Error;

#[derive(Debug, Error)]
pub enum Err {
    Cid(cid::Error),
}

pub type Result<T> = std::result::Result<T, Err>;

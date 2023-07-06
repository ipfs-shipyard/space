pub mod block;
pub mod error;
pub mod provider;
pub mod storage;
mod util;

mod null_provider;

#[cfg(feature = "files")]
mod file_provider;
#[cfg(feature = "sqlite")]
pub mod sql_provider;

#[cfg(all(not(test), feature = "sqlite", feature = "files"))]
compile_error! {"Outside of unit tests there's not a good reason to compile with multiple StorageProvider implementations."}

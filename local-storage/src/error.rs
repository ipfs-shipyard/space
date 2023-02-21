use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Block not found for CID {0}: {1}")]
    BlockNotFound(String, String),
    #[error("DAG incomplete {0}")]
    DagIncomplete(String),
}

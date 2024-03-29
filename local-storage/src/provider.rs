use crate::block::StoredBlock;
use anyhow::Result;
use cid::Cid;
use std::sync::{Arc, Mutex};

#[allow(unused_imports)]
use crate::null_provider::NullStorageProvider;

#[cfg(feature = "sqlite")]
use crate::sql_provider::SqliteStorageProvider;

#[cfg(feature = "files")]
use crate::file_provider::FileStorageProvider;

pub type Handle = Arc<Mutex<dyn StorageProvider + Send>>;

pub trait StorageProvider {
    // Import a stored block
    fn import_block(&mut self, block: &StoredBlock) -> Result<()>;
    // Requests a list of CIDs currently available in storage
    fn get_available_cids(&self) -> Result<Vec<String>>;
    // Requests the block associated with the given CID
    fn get_block_by_cid(&self, cid: &str) -> Result<StoredBlock>;
    // Requests the links associated with the given CID
    fn get_links_by_cid(&self, cid: &str) -> Result<Vec<String>>;
    fn list_available_dags(&self) -> Result<Vec<(String, String)>>;
    // Attaches filename to dag
    fn name_dag(&self, cid: &str, file_name: &str) -> Result<()>;
    fn get_name(&self, cid: &str) -> Result<String>;
    fn get_missing_cid_blocks(&self, cid: &str) -> Result<Vec<String>>;
    fn get_dag_blocks_by_window(
        &self,
        cid: &str,
        offset: u32,
        window_size: u32,
    ) -> Result<Vec<StoredBlock>>;
    fn get_all_dag_cids(
        &self,
        cid: &str,
        offset: Option<u32>,
        window_size: Option<u32>,
    ) -> Result<Vec<String>>;
    fn get_all_dag_blocks(&self, cid: &str) -> Result<Vec<StoredBlock>>;
    fn incremental_gc(&mut self) -> bool;
    fn has_cid(&self, cid: &Cid) -> bool;
    fn ack_cid(&self, cid: &Cid);
    fn get_dangling_cids(&self) -> Result<Vec<Cid>>;
}

pub fn default_storage_provider(_storage_path: &str, _high_disk_usage: u64) -> Result<Handle> {
    #[cfg(all(not(feature = "files"), not(feature = "sqlite")))]
    let provider = NullStorageProvider::default();
    #[cfg(all(feature = "files", not(feature = "sqlite")))]
    let provider = FileStorageProvider::new(_storage_path, _high_disk_usage)?;
    #[cfg(feature = "sqlite")]
    let provider = SqliteStorageProvider::new(_storage_path)?;
    Ok(Arc::new(Mutex::new(provider)))
}

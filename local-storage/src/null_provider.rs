use crate::block::StoredBlock;
use crate::provider::StorageProvider;
use anyhow::bail;
use cid::Cid;

#[derive(Default)]
pub(crate) struct NullStorageProvider {}

impl StorageProvider for NullStorageProvider {
    fn import_block(&mut self, _block: &StoredBlock) -> anyhow::Result<()> {
        bail!("NullStorageProvider does not implement anything")
    }
    fn get_dangling_cids(&self) -> anyhow::Result<Vec<Cid>> {
        Ok(vec![])
    }
    fn get_available_cids(&self) -> anyhow::Result<Vec<String>> {
        bail!("NullStorageProvider does not implement anything")
    }

    fn get_block_by_cid(&self, _cid: &str) -> anyhow::Result<StoredBlock> {
        bail!("NullStorageProvider does not implement anything")
    }

    fn get_links_by_cid(&self, _cid: &str) -> anyhow::Result<Vec<String>> {
        bail!("NullStorageProvider does not implement anything")
    }

    fn list_available_dags(&self) -> anyhow::Result<Vec<(String, String)>> {
        bail!("NullStorageProvider does not implement anything")
    }

    fn name_dag(&self, _cid: &str, _file_name: &str) -> anyhow::Result<()> {
        bail!("NullStorageProvider does not implement anything")
    }

    fn get_missing_cid_blocks(&self, _cid: &str) -> anyhow::Result<Vec<String>> {
        bail!("NullStorageProvider does not implement anything")
    }

    fn get_dag_blocks_by_window(
        &self,
        _cid: &str,
        _offset: u32,
        _window_size: u32,
    ) -> anyhow::Result<Vec<StoredBlock>> {
        bail!("NullStorageProvider does not implement anything")
    }

    fn get_all_dag_cids(
        &self,
        _cid: &str,
        _offset: Option<u32>,
        _window_size: Option<u32>,
    ) -> anyhow::Result<Vec<String>> {
        bail!("NullStorageProvider does not implement anything")
    }

    fn get_all_dag_blocks(&self, _cid: &str) -> anyhow::Result<Vec<StoredBlock>> {
        bail!("NullStorageProvider does not implement anything")
    }

    fn incremental_gc(&mut self) -> bool {
        false
    }

    fn has_cid(&self, _cid: &Cid) -> bool {
        false
    }

    fn ack_cid(&self, _cid: &Cid) {}
}

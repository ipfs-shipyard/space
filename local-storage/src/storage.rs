use crate::{block::StoredBlock, error::StorageError, provider::Handle as ProviderHandle};
use anyhow::{bail, Result};
use cid::Cid;
use futures::TryStreamExt;
use ipfs_unixfs::{
    builder::{File, FileBuilder},
    Block,
};
use std::sync::Arc;
use std::{fs::File as FsFile, io::Write, path::Path};

use log::{debug, error, info, trace};

pub struct Storage {
    provider: ProviderHandle,
    block_size: u32,
    degree: usize,
}

impl Storage {
    pub fn new(provider: ProviderHandle, block_size: u32) -> Self {
        let degree = ((block_size as usize - 8) / 50).clamp(
            //A stem in a tree must be allowed at least 2 links for it to be a tree
            2,
            //the default degree is also the spec-defined max
            ipfs_unixfs::balanced_tree::DEFAULT_DEGREE,
        );
        Storage {
            provider,
            block_size,
            degree,
        }
    }
    pub fn import_path(&mut self, path: &Path) -> Result<String> {
        debug!("import_path({:?})", &path);
        let rt = tokio::runtime::Runtime::new()?;
        let blocks: Result<Vec<Block>> = rt.block_on(async {
            let file: File = FileBuilder::new()
                .path(path)
                .fixed_chunker(self.block_size.try_into()?)
                .degree(self.degree)
                .build()
                .await?;
            let blocks: Vec<_> = file.encode().await?.try_collect().await?;
            Ok(blocks)
        });
        let blocks = blocks?;
        for block in &blocks {
            assert!(block.data().len() <= self.block_size as usize);
        }
        let mut root_cid: Option<String> = None;

        blocks.iter().for_each(|b| {
            let links = b
                .links()
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<String>>();
            let stored = StoredBlock {
                cid: b.cid().to_string(),
                data: b.data().to_vec(),
                links,
                filename: None,
            };
            // First validate each block
            if let Err(_e) = stored.validate() {
                error!("Failed to validate {}, {_e}", b.cid());
            }
            if let Err(_e) = self.provider.lock().unwrap().import_block(&stored) {
                error!("Failed to import block {_e}");
            }
            if !stored.links.is_empty() {
                root_cid = Some(stored.cid);
            }
        });
        if blocks.len() == 1 {
            if let Some(first) = blocks.first() {
                root_cid = Some(first.cid().to_string());
            }
        }
        if let Some(root_cid) = root_cid {
            if let Some(filename) = path.file_name().and_then(|p| p.to_str()) {
                let lck = self.provider.lock().unwrap();
                lck.name_dag(&root_cid, filename)?;
            }

            info!(
                "Imported path {} to {} in {} blocks",
                path.display(),
                root_cid,
                blocks.len()
            );
            Ok(root_cid)
        } else {
            bail!("Failed to find root block for {path:?}")
        }
    }

    pub fn export_cid(&self, cid: &str, path: &Path) -> Result<()> {
        let check_missing_blocks = self.get_missing_dag_blocks(cid)?;
        if !check_missing_blocks.is_empty() {
            error!(
                "Can't export {cid} to {}, because we're missing blocks: {:?}",
                path.display(),
                check_missing_blocks
            );
            bail!(StorageError::DagIncomplete(cid.to_string()))
        }
        // Fetch all blocks tied to links under given cid
        let child_blocks = self.get_all_dag_blocks(cid)?;
        debug!(
            "Planning to export {} child_blocks to {path:?}",
            child_blocks.len()
        );
        // Open up file path for writing
        let mut output_file = FsFile::create(path)?;
        // Walk the StoredBlocks and write out to path
        for block in child_blocks {
            if block.links.is_empty() {
                output_file.write_all(&block.data)?;
            }
        }
        output_file.sync_all()?;

        info!("Exported {cid} to {}", path.display());
        Ok(())
    }

    pub fn list_available_cids(&self) -> Result<Vec<String>> {
        // Query list of available CIDs
        // Include all root and child CIDs?
        self.provider.lock().unwrap().get_available_cids()
    }

    pub fn get_block_by_cid(&self, cid: &str) -> Result<StoredBlock> {
        // Check if CID+block exists
        // Return block if exists
        self.provider.lock().unwrap().get_block_by_cid(cid)
    }

    pub fn get_all_dag_cids(
        &self,
        cid: &str,
        offset: Option<u32>,
        window_size: Option<u32>,
    ) -> Result<Vec<String>> {
        self.provider
            .lock()
            .unwrap()
            .get_all_dag_cids(cid, offset, window_size)
    }

    pub fn get_all_dag_blocks(&self, cid: &str) -> Result<Vec<StoredBlock>> {
        self.provider.lock().unwrap().get_all_dag_blocks(cid)
    }

    pub fn import_block(&mut self, block: &StoredBlock) -> Result<()> {
        info!("Importing block {:?}", block);
        trace!(
            "Block to be imported ({}) links to {:?}",
            block.cid,
            block.links
        );
        self.provider.lock().unwrap().import_block(block)
    }

    pub fn get_missing_dag_blocks(&self, cid: &str) -> Result<Vec<String>> {
        self.provider.lock().unwrap().get_missing_cid_blocks(cid)
    }

    pub fn list_available_dags(&self) -> Result<Vec<(String, String)>> {
        self.provider.lock().unwrap().list_available_dags()
    }

    pub fn get_dag_blocks_by_window(
        &self,
        cid: &str,
        window_size: u32,
        window_num: u32,
    ) -> Result<Vec<StoredBlock>> {
        let offset = window_size * window_num;

        self.provider
            .lock()
            .unwrap()
            .get_dag_blocks_by_window(cid, offset, window_size)
    }

    pub fn incremental_gc(&mut self) -> bool {
        if let Ok(mut prov) = self.provider.lock() {
            prov.incremental_gc()
        } else {
            false
        }
    }
    pub fn has_cid(&self, cid: &Cid) -> bool {
        self.provider
            .lock()
            .map(|p| p.has_cid(cid))
            .unwrap_or(false)
    }
    pub fn ack_cid(&self, cid: &Cid) {
        if let Ok(prov) = self.provider.lock() {
            prov.ack_cid(cid)
        }
    }

    pub fn get_provider(&self) -> ProviderHandle {
        Arc::clone(&self.provider)
    }

    pub fn set_name(&self, cid: &str, name: &str) {
        if let Ok(prov) = self.provider.lock() {
            if let Err(e) = prov.name_dag(cid, name) {
                error!("Error: {e:?}");
            }
        }
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;
    use crate::sql_provider::SqliteStorageProvider;
    use assert_fs::{fixture::FileWriteBin, fixture::PathChild, TempDir};
    use rand::{thread_rng, RngCore};
    use std::sync::{Arc, Mutex};

    const BLOCK_SIZE: usize = 1024 * 10;

    struct TestHarness {
        storage: Storage,
        _db_dir: TempDir,
    }

    impl TestHarness {
        pub fn new() -> Self {
            let db_dir = TempDir::new().unwrap();
            let db_path = db_dir.child("storage.db");
            let provider = SqliteStorageProvider::new(db_path.path().to_str().unwrap()).unwrap();
            provider.setup().unwrap();
            let storage = Storage::new(
                Arc::new(Mutex::new(provider)),
                BLOCK_SIZE.try_into().unwrap(),
            );
            TestHarness {
                storage,
                _db_dir: db_dir,
            }
        }
    }

    #[test]
    pub fn test_import_path_to_storage_single_block() {
        let mut harness = TestHarness::new();

        let temp_dir = assert_fs::TempDir::new().unwrap();
        let test_file = temp_dir.child("data.txt");
        test_file
            .write_binary(
                "654684646847616846846876168468416874616846416846846186468464684684648684684"
                    .repeat(10)
                    .as_bytes(),
            )
            .unwrap();
        let root_cid = harness.storage.import_path(test_file.path()).unwrap();

        let available_cids = harness.storage.list_available_cids().unwrap();

        assert!(available_cids.contains(&root_cid));

        let available_dags = harness.storage.list_available_dags().unwrap();
        assert_eq!(available_dags, vec![(root_cid, "data.txt".to_string())]);
    }

    #[test]
    pub fn test_import_path_to_storage_multi_block() {
        let mut harness = TestHarness::new();

        let temp_dir = assert_fs::TempDir::new().unwrap();
        let test_file = temp_dir.child("data.txt");
        test_file
            .write_binary(
                "654684646847616846846876168468416874616846416846846186468464684684648684684"
                    .repeat(500)
                    .as_bytes(),
            )
            .unwrap();
        let root_cid = harness.storage.import_path(test_file.path()).unwrap();

        let available_cids = harness.storage.list_available_cids().unwrap();

        assert!(available_cids.contains(&root_cid));

        let available_dags = harness.storage.list_available_dags().unwrap();
        assert_eq!(available_dags, vec![(root_cid, "data.txt".to_string())]);
    }

    #[test]
    pub fn export_path_from_storage() {
        let mut harness = TestHarness::new();

        let temp_dir = assert_fs::TempDir::new().unwrap();
        let test_file = temp_dir.child("data.txt");
        test_file
            .write_binary(
                "654684646847616846846876168468416874616846416846846186468464684684648684684"
                    .repeat(500)
                    .as_bytes(),
            )
            .unwrap();
        let cid = harness.storage.import_path(test_file.path()).unwrap();

        let next_test_file = temp_dir.child("output.txt");
        harness
            .storage
            .export_cid(&cid, next_test_file.path())
            .unwrap();

        let test_file_contents = std::fs::read_to_string(test_file.path()).unwrap();
        let next_test_file_contents = std::fs::read_to_string(next_test_file.path()).unwrap();
        assert_eq!(test_file_contents, next_test_file_contents);
    }

    #[test]
    pub fn export_from_storage_various_file_sizes_binary_data() {
        for size in [100, 200, 300, 500, 1_000] {
            let mut harness = TestHarness::new();
            let temp_dir = assert_fs::TempDir::new().unwrap();
            let test_file = temp_dir.child("data.txt");

            let data_size = BLOCK_SIZE * size;
            let mut data = Vec::<u8>::new();
            data.resize(data_size, 1);
            thread_rng().fill_bytes(&mut data);

            test_file.write_binary(&data).unwrap();
            let cid = harness.storage.import_path(test_file.path()).unwrap();

            let next_test_file = temp_dir.child("output.txt");
            harness
                .storage
                .export_cid(&cid, next_test_file.path())
                .unwrap();

            let test_file_contents = std::fs::read(test_file.path()).unwrap();
            let next_test_file_contents = std::fs::read(next_test_file.path()).unwrap();
            assert_eq!(test_file_contents.len(), next_test_file_contents.len());
            assert_eq!(test_file_contents, next_test_file_contents);
        }
    }

    #[test]
    pub fn test_get_dag_blocks_by_window() {
        let mut harness = TestHarness::new();
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let test_file = temp_dir.child("data.txt");

        let data_size = BLOCK_SIZE * 50;
        let mut data = Vec::<u8>::new();
        data.resize(data_size, 1);
        thread_rng().fill_bytes(&mut data);

        test_file.write_binary(&data).unwrap();
        let cid = harness.storage.import_path(test_file.path()).unwrap();

        let window_size: u32 = 10;
        let all_dag_blocks = harness.storage.get_all_dag_blocks(&cid).unwrap();

        for (window_num, chunk) in all_dag_blocks.chunks(window_size as usize).enumerate() {
            let window_blocks = harness
                .storage
                .get_dag_blocks_by_window(&cid, window_size, window_num.try_into().unwrap())
                .unwrap();
            assert_eq!(chunk, &window_blocks);
        }
    }

    #[test]
    pub fn compare_get_blocks_to_get_cids() {
        let mut harness = TestHarness::new();
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let test_file = temp_dir.child("data.txt");

        let data_size = BLOCK_SIZE * 50;
        let mut data = Vec::<u8>::new();
        data.resize(data_size, 1);
        thread_rng().fill_bytes(&mut data);

        test_file.write_binary(&data).unwrap();
        let cid = harness.storage.import_path(test_file.path()).unwrap();

        let blocks = harness.storage.get_all_dag_blocks(&cid).unwrap();
        let cids = harness.storage.get_all_dag_cids(&cid, None, None).unwrap();

        assert_eq!(blocks.len(), cids.len());
    }

    // TODO: duplicated data is not being handled correctly right now, need to fix this
    // #[test]
    // pub fn export_from_storage_various_file_sizes_duplicated_data() {
    //     for size in [100, 200, 300, 500, 1000] {
    //         let mut harness = TestHarness::new();
    //         let temp_dir = assert_fs::TempDir::new().unwrap();
    //         let test_file = temp_dir.child("data.txt");
    //         test_file
    //             .write_binary(
    //                 "654684646847616846846876168468416874616846416846846186468464684684648684684"
    //                     .repeat(size)
    //                     .as_bytes(),
    //             )
    //             .unwrap();
    //         let cid = harness.storage.import_path(test_file.path()).unwrap();

    //         let next_test_file = temp_dir.child("output.txt");
    //         harness
    //             .storage
    //             .export_cid(&cid, next_test_file.path())
    //             .unwrap();

    //         let test_file_contents = std::fs::read(test_file.path()).unwrap();
    //         let next_test_file_contents = std::fs::read(next_test_file.path()).unwrap();
    //         assert_eq!(test_file_contents.len(), next_test_file_contents.len());
    //         assert_eq!(test_file_contents, next_test_file_contents);
    //     }
    // }
}

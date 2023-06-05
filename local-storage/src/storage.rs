use crate::error::StorageError;
use crate::provider::StorageProvider;

use crate::block::StoredBlock;
use anyhow::{bail, Result};
use futures::TryStreamExt;
use ipfs_unixfs::{
    builder::{File, FileBuilder},
    Block,
};
use std::fs::File as FsFile;
use std::io::Write;
use std::path::Path;
use tracing::{error, info};

pub struct Storage {
    pub provider: Box<dyn StorageProvider>,
    block_size: u32,
}

impl Storage {
    pub fn new(provider: Box<dyn StorageProvider>, block_size: u32) -> Self {
        Storage {
            provider,
            block_size,
        }
    }

    pub fn import_path(&self, path: &Path) -> Result<String> {
        let rt = tokio::runtime::Runtime::new()?;
        let blocks: Result<Vec<Block>> = rt.block_on(async {
            let file: File = FileBuilder::new()
                .path(path)
                .fixed_chunker(self.block_size.try_into()?)
                .build()
                .await?;
            let blocks: Vec<_> = file.encode().await?.try_collect().await?;
            Ok(blocks)
        });
        let blocks = blocks?;
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
            };
            // First validate each block
            if let Err(e) = stored.validate() {
                error!("Failed to validate {}, {e}", b.cid());
            }
            if let Err(e) = self.provider.import_block(&stored) {
                error!("Failed to import block {e}");
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
                println!("name dag {root_cid} -> {filename}");
                self.provider.name_dag(&root_cid, filename)?;
            }
            info!("Imported path {} to {}", path.display(), root_cid);
            info!("Importing {} blocks for {root_cid}", blocks.len());
            Ok(root_cid)
        } else {
            bail!("Failed to find root block for {path:?}")
        }
    }

    pub fn export_cid(&self, cid: &str, path: &Path) -> Result<()> {
        info!("Exporting {cid} to {}", path.display());
        let check_missing_blocks = self.get_missing_dag_blocks(cid)?;
        if !check_missing_blocks.is_empty() {
            bail!(StorageError::DagIncomplete(cid.to_string()))
        }
        // Fetch all blocks tied to links under given cid
        let child_blocks = self.get_all_dag_blocks(cid)?;
        // Open up file path for writing
        let mut output_file = FsFile::create(path)?;
        // Walk the StoredBlocks and write out to path
        for block in child_blocks {
            if block.links.is_empty() {
                output_file.write_all(&block.data)?;
            }
        }
        output_file.sync_all()?;
        Ok(())
    }

    pub fn list_available_cids(&self) -> Result<Vec<String>> {
        // Query list of available CIDs
        // Include all root and child CIDs?
        self.provider.get_available_cids()
    }

    pub fn get_block_by_cid(&self, cid: &str) -> Result<StoredBlock> {
        // Check if CID+block exists
        // Return block if exists
        self.provider.get_block_by_cid(cid)
    }

    pub fn get_all_dag_cids(&self, cid: &str) -> Result<Vec<String>> {
        self.provider.get_all_dag_cids(cid)
    }

    pub fn get_all_dag_blocks(&self, cid: &str) -> Result<Vec<StoredBlock>> {
        self.provider.get_all_dag_blocks(cid)
    }

    pub fn import_block(&self, block: &StoredBlock) -> Result<()> {
        info!("Importing block {}", block.cid);
        self.provider.import_block(block)
    }

    pub fn get_missing_dag_blocks(&self, cid: &str) -> Result<Vec<String>> {
        self.provider.get_missing_cid_blocks(cid)
    }

    pub fn list_available_dags(&self) -> Result<Vec<(String, String)>> {
        self.provider.list_available_dags()
    }

    pub fn get_dag_blocks_by_window(
        &self,
        cid: &str,
        window_size: u32,
        window_num: u32,
    ) -> Result<Vec<StoredBlock>> {
        let offset = window_size * window_num;

        self.provider
            .get_dag_blocks_by_window(cid, offset, window_size)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::provider::SqliteStorageProvider;
    use assert_fs::{fixture::FileWriteBin, fixture::PathChild, TempDir};
    use rand::{thread_rng, RngCore};

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
            let storage = Storage::new(Box::new(provider), BLOCK_SIZE.try_into().unwrap());
            TestHarness {
                storage,
                _db_dir: db_dir,
            }
        }
    }

    #[test]
    pub fn test_import_path_to_storage_single_block() {
        let harness = TestHarness::new();

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
        let harness = TestHarness::new();

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
        let harness = TestHarness::new();

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
            let harness = TestHarness::new();
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
        let harness = TestHarness::new();
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
        let harness = TestHarness::new();
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let test_file = temp_dir.child("data.txt");

        let data_size = BLOCK_SIZE * 50;
        let mut data = Vec::<u8>::new();
        data.resize(data_size, 1);
        thread_rng().fill_bytes(&mut data);

        test_file.write_binary(&data).unwrap();
        let cid = harness.storage.import_path(test_file.path()).unwrap();

        let blocks = harness.storage.get_all_dag_blocks(&cid).unwrap();
        let cids = harness.storage.get_all_dag_cids(&cid).unwrap();

        assert_eq!(blocks.len(), cids.len());
    }

    // TODO: duplicated data is not being handled correctly right now, need to fix this
    // #[test]
    // pub fn export_from_storage_various_file_sizes_duplicated_data() {
    //     for size in [100, 200, 300, 500, 1000] {
    //         let harness = TestHarness::new();
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

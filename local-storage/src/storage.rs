use crate::error::StorageError;
use crate::provider::StorageProvider;

use crate::block::StoredBlock;
use anyhow::{bail, Result};
use futures::TryStreamExt;
use iroh_unixfs::builder::{File, FileBuilder};
use std::path::Path;
use tokio::fs::File as TokioFile;
use tokio::io::AsyncWriteExt;
use tracing::{error, info};

pub struct Storage {
    pub provider: Box<dyn StorageProvider>,
}

impl Storage {
    pub fn new(provider: Box<dyn StorageProvider>) -> Self {
        Storage { provider }
    }

    pub async fn import_path(&self, path: &Path) -> Result<String> {
        let file: File = FileBuilder::new()
            .path(path)
            .fixed_chunker(50)
            .build()
            .await?;
        let blocks: Vec<_> = file.encode().await?.try_collect().await?;
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
            if let Err(e) = self.provider.import_block(&stored) {
                error!("Failed to import block {e}");
            }
            if !stored.links.is_empty() {
                root_cid = Some(stored.cid);
            }
        });
        if let Some(root_cid) = root_cid {
            info!("Imported path {} to {}", path.display(), root_cid);
            Ok(root_cid)
        } else {
            bail!("Failed to find root block for {path:?}")
        }
    }

    pub async fn export_cid(&self, cid: &str, path: &Path) -> Result<()> {
        info!("Exporting {cid} to {}", path.display());
        let check_missing_blocks = self.get_missing_dag_blocks(cid)?;
        if !check_missing_blocks.is_empty() {
            bail!(StorageError::DagIncomplete(cid.to_string()))
        }
        // Fetch all blocks tied to links under given cid
        let child_blocks = self.get_all_blocks_under_cid(cid)?;
        // Open up file path for writing
        let mut output_file = TokioFile::create(path).await?;
        // Walk the StoredBlocks and write out to path
        for block in child_blocks {
            output_file.write_all(&block.data).await?;
        }
        output_file.flush().await?;
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

    pub fn get_all_blocks_under_cid(&self, cid: &str) -> Result<Vec<StoredBlock>> {
        // Get StoredBlock by cid and check for links
        let root_block = self.provider.get_block_by_cid(cid)?;
        // If links, grab all appropriate StoredBlocks
        let mut child_blocks = vec![];
        for link in root_block.links {
            child_blocks.push(self.provider.get_block_by_cid(&link)?);
        }
        Ok(child_blocks)
    }

    pub fn import_block(&self, block: &StoredBlock) -> Result<()> {
        info!("Importing block {}", block.cid);
        self.provider.import_block(block)
    }

    pub fn get_missing_dag_blocks(&self, cid: &str) -> Result<Vec<String>> {
        self.provider.get_missing_cid_blocks(cid)
    }

    pub fn list_available_dags(&self) -> Result<Vec<String>> {
        self.provider.list_available_dags()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::provider::SqliteStorageProvider;
    use assert_fs::{fixture::FileWriteBin, fixture::PathChild, TempDir};

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
            let storage = Storage::new(Box::new(provider));
            return TestHarness {
                storage,
                _db_dir: db_dir,
            };
        }
    }

    #[tokio::test]
    pub async fn test_import_path_to_storage() {
        let harness = TestHarness::new();

        let temp_dir = assert_fs::TempDir::new().unwrap();
        let test_file = temp_dir.child("data.txt");
        test_file
            .write_binary(
                b"654684646847616846846876168468416874616846416846846186468464684684648684684",
            )
            .unwrap();
        let root_cid = harness.storage.import_path(test_file.path()).await.unwrap();

        let available_cids = harness.storage.list_available_cids().unwrap();

        assert!(available_cids.contains(&root_cid));
    }

    #[tokio::test]
    pub async fn export_path_from_storage() {
        let harness = TestHarness::new();

        let temp_dir = assert_fs::TempDir::new().unwrap();
        let test_file = temp_dir.child("data.txt");
        test_file
            .write_binary(
                b"654684646847616846846876168468416874616846416846846186468464684684648684684",
            )
            .unwrap();
        let cid = harness.storage.import_path(test_file.path()).await.unwrap();

        let next_test_file = temp_dir.child("output.txt");
        harness
            .storage
            .export_cid(&cid, next_test_file.path())
            .await
            .unwrap();

        let test_file_contents = std::fs::read_to_string(test_file.path()).unwrap();
        let next_test_file_contents = std::fs::read_to_string(next_test_file.path()).unwrap();
        assert_eq!(test_file_contents, next_test_file_contents);
    }
}

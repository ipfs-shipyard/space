use crate::transmit::transmit_blocks;
use anyhow::Result;
use local_storage::storage::Storage;
use messages::{ApplicationAPI, Message};
use std::path::PathBuf;
use std::rc::Rc;

pub async fn transmit_file(path: &str, target_addr: &str, storage: Rc<Storage>) -> Result<()> {
    let cid = storage.import_path(&PathBuf::from(path.to_owned())).await?;
    let root_block = storage.get_block_by_cid(&cid)?;
    let blocks = storage.get_all_blocks_under_cid(&cid)?;
    let mut all_blocks = vec![root_block];
    all_blocks.extend(blocks);
    transmit_blocks(&all_blocks, target_addr).await?;
    Ok(())
}

pub async fn transmit_dag(cid: &str, target_addr: &str, storage: Rc<Storage>) -> Result<()> {
    let root_block = storage.get_block_by_cid(cid)?;
    let blocks = storage.get_all_blocks_under_cid(cid)?;
    let mut all_blocks = vec![root_block];
    all_blocks.extend(blocks);
    transmit_blocks(&all_blocks, target_addr).await?;
    Ok(())
}

pub async fn import_file(path: &str, storage: Rc<Storage>) -> Result<Message> {
    let root_cid = storage.import_path(&PathBuf::from(path.to_owned())).await?;
    Ok(Message::ApplicationAPI(ApplicationAPI::FileImported {
        path: path.to_string(),
        cid: root_cid,
    }))
}

pub fn validate_dag(cid: &str, storage: Rc<Storage>) -> Result<Message> {
    let dag_blocks = storage.get_dag_blocks(cid)?;
    let resp = match local_storage::block::validate_dag(&dag_blocks) {
        Ok(_) => "Dag is valid".to_string(),
        Err(e) => e.to_string(),
    };
    Ok(Message::ApplicationAPI(
        ApplicationAPI::ValidateDagResponse {
            cid: cid.to_string(),
            result: resp,
        },
    ))
}

pub fn request_available_blocks(storage: Rc<Storage>) -> Result<Message> {
    let raw_cids = storage.list_available_cids()?;
    let cids = raw_cids
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<String>>();
    Ok(Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
        cids,
    }))
}

pub fn get_missing_dag_blocks(cid: &str, storage: Rc<Storage>) -> Result<Message> {
    let blocks = storage.get_missing_dag_blocks(cid)?;
    Ok(Message::ApplicationAPI(ApplicationAPI::MissingDagBlocks {
        blocks,
    }))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use assert_fs::{fixture::FileWriteBin, fixture::PathChild, TempDir};
    use futures::TryStreamExt;
    use iroh_unixfs::builder::{File, FileBuilder};
    use local_storage::block::StoredBlock;
    use local_storage::provider::SqliteStorageProvider;
    use rand::{thread_rng, RngCore};

    struct TestHarness {
        storage: Rc<Storage>,
        db_dir: TempDir,
    }

    impl TestHarness {
        pub fn new() -> Self {
            let db_dir = TempDir::new().unwrap();
            let db_path = db_dir.child("storage.db");
            let provider = SqliteStorageProvider::new(db_path.path().to_str().unwrap()).unwrap();
            provider.setup().unwrap();
            let storage = Rc::new(Storage::new(Box::new(provider)));
            return TestHarness { storage, db_dir };
        }

        pub fn generate_file(&self) -> Result<String> {
            let mut data = Vec::<u8>::new();
            data.resize(256, 1);
            thread_rng().fill_bytes(&mut data);

            let tmp_file = self.db_dir.child("test.file");
            tmp_file.write_binary(&data)?;
            Ok(tmp_file.path().to_str().unwrap().to_owned())
        }
    }

    async fn file_to_blocks(path: &str) -> Result<Vec<StoredBlock>> {
        let file: File = FileBuilder::new()
            .path(path)
            .fixed_chunker(50)
            .build()
            .await?;
        let blocks: Vec<_> = file.encode().await?.try_collect().await?;
        let mut stored_blocks = vec![];

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

            stored_blocks.push(stored);
        });

        Ok(stored_blocks)
    }

    #[tokio::test]
    pub async fn test_import_file_validate_dag() {
        let harness = TestHarness::new();

        let test_file_path = harness.generate_file().unwrap();

        let imported_file_cid = match import_file(&test_file_path, harness.storage.clone()).await {
            Ok(Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. })) => cid,
            other => panic!("ImportFile returned wrong response {other:?}"),
        };

        let (validated_cid, result) =
            match validate_dag(&imported_file_cid, harness.storage.clone()) {
                Ok(Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
                    cid,
                    result,
                })) => (cid, result),
                other => panic!("ValidateDag returned wrong response {other:?}"),
            };

        assert_eq!(imported_file_cid, validated_cid);
        assert_eq!(result, "Dag is valid");
    }

    #[tokio::test]
    pub async fn test_import_file_validate_blocks() {
        let harness = TestHarness::new();

        let test_file_path = harness.generate_file().unwrap();

        let imported_file_cid = match import_file(&test_file_path, harness.storage.clone()).await {
            Ok(Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. })) => cid,
            other => panic!("ImportFile returned wrong response {other:?}"),
        };

        let blocks = harness
            .storage
            .get_all_blocks_under_cid(&imported_file_cid)
            .unwrap();
        for block in blocks {
            let (validated_cid, result) = match validate_dag(&block.cid, harness.storage.clone()) {
                Ok(Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
                    cid,
                    result,
                })) => (cid, result),
                other => panic!("ValidateDag returned wrong response {other:?}"),
            };

            assert_eq!(block.cid, validated_cid);
            assert_eq!(result, "Dag is valid");
        }
    }

    #[tokio::test]
    pub async fn test_available_blocks() {
        let harness = TestHarness::new();

        let available_blocks = match request_available_blocks(harness.storage.clone()) {
            Ok(Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids })) => cids,
            other => panic!("RequestAvailableBlocks returned wrong response: {other:?}"),
        };
        assert!(available_blocks.is_empty());

        let test_file_path = harness.generate_file().unwrap();
        import_file(&test_file_path, harness.storage.clone())
            .await
            .unwrap();

        let available_blocks = match request_available_blocks(harness.storage.clone()) {
            Ok(Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids })) => cids,
            other => panic!("RequestAvailableBlocks returned wrong response: {other:?}"),
        };
        let storage_available_blocks = harness.storage.list_available_cids().unwrap();
        assert_eq!(available_blocks, storage_available_blocks);
    }

    #[tokio::test]
    pub async fn test_get_missing_blocks_none_missing() {
        let harness = TestHarness::new();

        let test_file_path = harness.generate_file().unwrap();

        let imported_file_cid = match import_file(&test_file_path, harness.storage.clone()).await {
            Ok(Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. })) => cid,
            other => panic!("ImportFile returned wrong response {other:?}"),
        };

        let missing_blocks =
            match get_missing_dag_blocks(&imported_file_cid, harness.storage.clone()) {
                Ok(Message::ApplicationAPI(ApplicationAPI::MissingDagBlocks { blocks })) => blocks,
                other => panic!("GetMissingDagBlocks returned wrong response: {other:?}"),
            };

        assert!(missing_blocks.is_empty());
    }

    #[tokio::test]
    pub async fn test_get_missing_blocks_one_missing() {
        let harness = TestHarness::new();

        let test_file_path = harness.generate_file().unwrap();
        let mut file_blocks = file_to_blocks(&test_file_path).await.unwrap();
        let missing_block = file_blocks.remove(0);
        let root_cid = file_blocks.last().unwrap().cid.to_owned();

        for block in file_blocks {
            harness.storage.import_block(&block).unwrap();
        }

        let missing_blocks = match get_missing_dag_blocks(&root_cid, harness.storage.clone()) {
            Ok(Message::ApplicationAPI(ApplicationAPI::MissingDagBlocks { blocks })) => blocks,
            other => panic!("GetMissingDagBlocks returned wrong response: {other:?}"),
        };

        assert_eq!(missing_blocks, vec![missing_block.cid]);
    }
}

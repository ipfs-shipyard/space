use anyhow::Result;
use local_storage::storage::Storage;
use messages::{ApplicationAPI, DagInfo, DataProtocol, Message};
use std::path::PathBuf;
use std::rc::Rc;

pub fn import_file(path: &str, storage: Rc<Storage>) -> Result<Message> {
    let root_cid = storage.import_path(&PathBuf::from(path.to_owned()))?;
    Ok(Message::ApplicationAPI(ApplicationAPI::FileImported {
        path: path.to_string(),
        cid: root_cid,
    }))
}

pub fn validate_dag(cid: &str, storage: Rc<Storage>) -> Result<Message> {
    let dag_blocks = storage.get_all_dag_blocks(cid)?;
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
        cid: cid.to_string(),
        blocks,
    }))
}

pub fn get_missing_dag_blocks_window_protocol(
    cid: &str,
    blocks: Vec<String>,
    storage: Rc<Storage>,
) -> Result<Message> {
    let mut missing_blocks = vec![];
    for block in blocks {
        if storage.get_block_by_cid(&block).is_err() {
            missing_blocks.push(block);
        }
    }

    Ok(Message::DataProtocol(DataProtocol::MissingDagBlocks {
        cid: cid.to_string(),
        blocks: missing_blocks,
    }))
}

pub fn get_available_dags(storage: Rc<Storage>) -> Result<Message> {
    let local_dags: Vec<DagInfo> = storage
        .list_available_dags()?
        .iter()
        .map(|(cid, filename)| DagInfo {
            cid: cid.to_string(),
            filename: filename.to_string(),
        })
        .collect();
    Ok(Message::ApplicationAPI(ApplicationAPI::AvailableDags {
        dags: local_dags,
    }))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use assert_fs::{fixture::FileWriteBin, fixture::PathChild, TempDir};
    use futures::TryStreamExt;
    use ipfs_unixfs::builder::{File, FileBuilder};
    use local_storage::block::StoredBlock;
    use local_storage::provider::SqliteStorageProvider;
    use rand::{thread_rng, RngCore};

    const BLOCK_SIZE: u32 = 1024 * 3;

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
            let storage = Rc::new(Storage::new(Box::new(provider), BLOCK_SIZE));
            TestHarness { storage, db_dir }
        }

        pub fn generate_file(&self) -> Result<String> {
            let mut data = Vec::<u8>::new();
            data.resize(256 * 5, 1);
            thread_rng().fill_bytes(&mut data);

            let tmp_file = self.db_dir.child("test.file");
            tmp_file.write_binary(&data)?;
            Ok(tmp_file.path().to_str().unwrap().to_owned())
        }
    }

    fn file_to_blocks(path: &str) -> Result<Vec<StoredBlock>> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let blocks = rt.block_on(async {
            let file: File = FileBuilder::new()
                .path(path)
                .fixed_chunker(50)
                .build()
                .await
                .unwrap();
            let blocks: Vec<_> = file.encode().await.unwrap().try_collect().await.unwrap();
            blocks
        });
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

    #[test]
    pub fn test_import_file_validate_dag() {
        let harness = TestHarness::new();

        let test_file_path = harness.generate_file().unwrap();

        let imported_file_cid = match import_file(&test_file_path, harness.storage.clone()) {
            Ok(Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. })) => cid,
            other => panic!("ImportFile returned wrong response {other:?}"),
        };

        let (validated_cid, result) = match validate_dag(&imported_file_cid, harness.storage) {
            Ok(Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse { cid, result })) => {
                (cid, result)
            }
            other => panic!("ValidateDag returned wrong response {other:?}"),
        };

        assert_eq!(imported_file_cid, validated_cid);
        assert_eq!(result, "Dag is valid");
    }

    #[test]
    pub fn test_import_file_validate_blocks() {
        let harness = TestHarness::new();

        let test_file_path = harness.generate_file().unwrap();

        let imported_file_cid = match import_file(&test_file_path, harness.storage.clone()) {
            Ok(Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. })) => cid,
            other => panic!("ImportFile returned wrong response {other:?}"),
        };

        let blocks = harness
            .storage
            .get_all_dag_blocks(&imported_file_cid)
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

    #[test]
    pub fn test_available_blocks() {
        let harness = TestHarness::new();

        let available_blocks = match request_available_blocks(harness.storage.clone()) {
            Ok(Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids })) => cids,
            other => panic!("RequestAvailableBlocks returned wrong response: {other:?}"),
        };
        assert!(available_blocks.is_empty());

        let test_file_path = harness.generate_file().unwrap();
        import_file(&test_file_path, harness.storage.clone()).unwrap();

        let available_blocks = match request_available_blocks(harness.storage.clone()) {
            Ok(Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids })) => cids,
            other => panic!("RequestAvailableBlocks returned wrong response: {other:?}"),
        };
        let storage_available_blocks = harness.storage.list_available_cids().unwrap();
        assert_eq!(available_blocks, storage_available_blocks);
    }

    #[test]
    pub fn test_get_missing_blocks_none_missing() {
        let harness = TestHarness::new();

        let test_file_path = harness.generate_file().unwrap();

        let imported_file_cid = match import_file(&test_file_path, harness.storage.clone()) {
            Ok(Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. })) => cid,
            other => panic!("ImportFile returned wrong response {other:?}"),
        };

        let missing_blocks = match get_missing_dag_blocks(&imported_file_cid, harness.storage) {
            Ok(Message::ApplicationAPI(ApplicationAPI::MissingDagBlocks { blocks, .. })) => blocks,
            other => panic!("GetMissingDagBlocks returned wrong response: {other:?}"),
        };

        assert!(missing_blocks.is_empty());
    }

    #[test]
    pub fn test_get_missing_blocks_one_missing() {
        let harness = TestHarness::new();

        let test_file_path = harness.generate_file().unwrap();
        let mut file_blocks = file_to_blocks(&test_file_path).unwrap();
        let missing_block = file_blocks.remove(0);
        let root_cid = file_blocks.last().unwrap().cid.to_owned();

        for block in file_blocks {
            harness.storage.import_block(&block).unwrap();
        }

        let missing_blocks = match get_missing_dag_blocks(&root_cid, harness.storage) {
            Ok(Message::ApplicationAPI(ApplicationAPI::MissingDagBlocks { blocks, .. })) => blocks,
            other => panic!("GetMissingDagBlocks returned wrong response: {other:?}"),
        };

        assert_eq!(missing_blocks, vec![missing_block.cid]);
    }
}

use anyhow::Result;
use block_ship::{types::BlockWrapper, types::CID_MARKER_LEN};
use local_storage::storage::{Storage, StoredBlock};
use messages::{TransmissionChunk, TransmissionMessage};
use std::collections::BTreeMap;
use std::rc::Rc;
use tracing::info;

pub struct Receiver {
    // Map of cid_marker to cid
    pub cids_to_build: BTreeMap<Vec<u8>, Vec<u8>>,
    // Map of cid_marker to [chunk]
    pub cid_chunks: BTreeMap<Vec<u8>, Vec<TransmissionChunk>>,
    pub storage: Rc<Storage>,
}

impl Receiver {
    pub fn new(storage: Rc<Storage>) -> Receiver {
        Receiver {
            cids_to_build: BTreeMap::new(),
            cid_chunks: BTreeMap::new(),
            storage,
        }
    }

    pub fn handle_chunk_msg(&mut self, chunk: TransmissionChunk) -> Result<()> {
        info!(
            "Received chunk {} for CID: {:?}",
            chunk.chunk_offset, chunk.cid_marker
        );

        self.cid_chunks
            .entry(chunk.cid_marker.clone())
            .and_modify(|vec| vec.push(chunk.clone()))
            .or_insert(vec![chunk.clone()]);
        Ok(())
    }

    pub fn handle_cid_msg(&mut self, cid: Vec<u8>) -> Result<()> {
        let cid_marker = &cid[..CID_MARKER_LEN];
        self.cids_to_build.entry(cid_marker.to_vec()).or_insert(cid);
        Ok(())
    }

    pub fn attempt_block_assembly(&mut self) -> Result<()> {
        for (cid_marker, cid) in &self.cids_to_build {
            if let Some(cid_chunks) = &self.cid_chunks.get(cid_marker) {
                if let Ok(wrapper) = BlockWrapper::from_chunks(cid, cid_chunks) {
                    let stored_block: StoredBlock = wrapper.try_into()?;
                    info!(
                        "Found block {} with {} bytes {} links",
                        &stored_block.cid,
                        &stored_block.data.len(),
                        &stored_block.links.len()
                    );
                    self.storage.import_block(&stored_block)?;
                    // Purge chunks after insert
                    self.cid_chunks.remove(cid_marker);
                }
            }
        }
        Ok(())
    }

    pub async fn handle_transmission_msg(&mut self, msg: TransmissionMessage) -> Result<()> {
        match msg {
            TransmissionMessage::Chunk(chunk) => self.handle_chunk_msg(chunk)?,
            TransmissionMessage::Cid(cid) => self.handle_cid_msg(cid)?,
        }
        self.attempt_block_assembly()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use assert_fs::{fixture::PathChild, TempDir};
    use block_ship::types::{BlockPayload, BlockWrapper};
    use cid::multihash::{Code, MultihashDigest};
    use cid::Cid;
    use futures::TryStreamExt;
    use iroh_unixfs::builder::{File, FileBuilder};
    use local_storage::provider::SqliteStorageProvider;

    struct TestHarness {
        storage: Rc<Storage>,
        receiver: Receiver,
        _db_dir: TempDir,
    }

    impl TestHarness {
        pub fn new() -> Self {
            let db_dir = TempDir::new().unwrap();
            let db_path = db_dir.child("storage.db");
            let provider = SqliteStorageProvider::new(db_path.path().to_str().unwrap()).unwrap();
            provider.setup().unwrap();
            let storage = Rc::new(Storage::new(Box::new(provider)));
            let receiver = Receiver::new(Rc::clone(&storage));
            return TestHarness {
                storage,
                receiver,
                _db_dir: db_dir,
            };
        }
    }

    #[test]
    pub fn test_receive_chunk() {
        let mut harness = TestHarness::new();

        let chunk = TransmissionChunk {
            cid_marker: vec![],
            chunk_offset: 0,
            data: vec![],
        };
        harness.receiver.handle_chunk_msg(chunk.clone()).unwrap();
        let entry = harness.receiver.cid_chunks.first_key_value().unwrap();
        assert_eq!(entry.0, &chunk.cid_marker);
        assert!(entry.1.contains(&chunk));
    }

    #[test]
    pub fn test_receive_cid() {
        let mut harness = TestHarness::new();
        let cid = b"101010101010101".to_vec();
        harness.receiver.handle_cid_msg(cid.clone()).unwrap();

        assert_eq!(
            harness.receiver.cids_to_build.first_key_value().unwrap().1,
            &cid
        );
    }

    #[tokio::test]
    pub async fn test_child_block_assembly() {
        let mut harness = TestHarness::new();
        let h = Code::Sha2_256.digest(b"test this");
        let cid = Cid::new_v1(0x55, h);

        let wrapper = BlockWrapper {
            cid: cid.to_bytes(),
            payload: BlockPayload {
                data: b"11111".to_vec(),
                links: vec![],
            },
        };
        let chunks = wrapper.to_chunks().unwrap();
        for c in chunks {
            harness.receiver.handle_transmission_msg(c).await.unwrap();
        }

        harness.receiver.attempt_block_assembly().unwrap();
        assert_eq!(harness.storage.list_available_cids().unwrap().len(), 1);
    }

    #[tokio::test]
    pub async fn test_root_block_assembly() {
        let mut harness = TestHarness::new();
        let h = Code::Sha2_256.digest(b"test this");
        let cid = Cid::new_v1(0x55, h);

        let wrapper = BlockWrapper {
            cid: cid.to_bytes(),
            payload: BlockPayload {
                data: b"11111".to_vec(),
                links: vec![cid.to_bytes()],
            },
        };
        let chunks = wrapper.to_chunks().unwrap();
        for c in chunks {
            harness.receiver.handle_transmission_msg(c).await.unwrap();
        }

        harness.receiver.attempt_block_assembly().unwrap();
        assert_eq!(harness.storage.list_available_dags().unwrap().len(), 1);
    }

    // TODO: write tests for handling incomplete blocks

    // TODO: implement support for handling single block files
    #[ignore]
    #[tokio::test]
    pub async fn test_verify_single_block_complete() {
        let mut harness = TestHarness::new();
        let content = b"10101010101001010101010".to_vec();
        let file: File = FileBuilder::new()
            .content_bytes(content)
            .name("test-name")
            .fixed_chunker(50)
            .build()
            .await
            .unwrap();

        let blocks: Vec<_> = file.encode().await.unwrap().try_collect().await.unwrap();
        assert_eq!(blocks.len(), 1);
        let mut msgs = vec![];
        for b in blocks {
            let wrapper = BlockWrapper::from(b);
            let chunks = wrapper.to_chunks().unwrap();
            msgs.extend(chunks);
        }

        for m in msgs {
            harness.receiver.handle_transmission_msg(m).await.unwrap();
        }
        let root = harness.storage.list_available_dags().unwrap();
        let root = root.first().unwrap();
        assert_eq!(
            harness.storage.get_missing_dag_blocks(root).unwrap().len(),
            0
        );
    }

    #[tokio::test]
    pub async fn test_verify_multi_block_complete_with_leading_root_block() {
        let mut harness = TestHarness::new();
        let content =
            b"1010101010100101010101010101010101010101010101010101010101001010101010101010"
                .to_vec();
        let file: File = FileBuilder::new()
            .content_bytes(content)
            .name("test-name")
            .fixed_chunker(50)
            .build()
            .await
            .unwrap();

        let mut blocks: Vec<_> = file.encode().await.unwrap().try_collect().await.unwrap();
        // Reverse to get root block at the front
        blocks.reverse();
        assert_eq!(blocks.len(), 3);
        let mut msgs = vec![];
        for b in blocks {
            let wrapper = BlockWrapper::from(b);
            let chunks = wrapper.to_chunks().unwrap();
            msgs.extend(chunks);
        }

        for m in msgs {
            harness.receiver.handle_transmission_msg(m).await.unwrap();
        }
        let blocks = harness.storage.list_available_cids().unwrap();
        assert_eq!(blocks.len(), 3);
        let root = harness.storage.list_available_dags().unwrap();
        let root = root.first().unwrap();
        assert_eq!(
            harness.storage.get_missing_dag_blocks(root).unwrap().len(),
            0
        );
    }

    #[tokio::test]
    pub async fn test_verify_multi_block_complete_with_trailing_root_block() {
        let mut harness = TestHarness::new();
        let content =
            b"1010101010100101010101010101010101010101010101010101010101001010101010101010"
                .to_vec();
        let file: File = FileBuilder::new()
            .content_bytes(content)
            .name("test-name")
            .fixed_chunker(50)
            .build()
            .await
            .unwrap();

        let blocks: Vec<_> = file.encode().await.unwrap().try_collect().await.unwrap();
        assert_eq!(blocks.len(), 3);
        let mut msgs = vec![];
        for b in blocks {
            let wrapper = BlockWrapper::from(b);
            let chunks = wrapper.to_chunks().unwrap();
            msgs.extend(chunks);
        }

        for m in msgs {
            harness.receiver.handle_transmission_msg(m).await.unwrap();
        }
        let blocks = harness.storage.list_available_cids().unwrap();
        assert_eq!(blocks.len(), 3);
        let root = harness.storage.list_available_dags().unwrap();
        let root = root.first().unwrap();
        assert_eq!(
            harness.storage.get_missing_dag_blocks(root).unwrap().len(),
            0
        );
    }
}

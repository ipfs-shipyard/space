use anyhow::Result;
use cid::Cid;
use local_storage::block::StoredBlock;
use local_storage::storage::Storage;
use messages::{DataProtocol, TransmissionBlock};
use std::rc::Rc;

pub struct Receiver {
    // Handle to Storage
    pub storage: Rc<Storage>,
}

impl Receiver {
    pub fn new(storage: Rc<Storage>) -> Receiver {
        Receiver { storage }
    }

    pub fn handle_block_msg(&mut self, block: TransmissionBlock) -> Result<()> {
        let mut links = vec![];
        for l in block.links {
            links.push(Cid::try_from(l)?.to_string());
        }
        let stored_block = StoredBlock {
            cid: Cid::try_from(block.cid)?.to_string(),
            data: block.data,
            links,
        };
        stored_block.validate()?;
        self.storage.import_block(&stored_block)
    }

    pub async fn handle_transmission_msg(&mut self, msg: DataProtocol) -> Result<()> {
        match msg {
            DataProtocol::Block(block) => self.handle_block_msg(block)?,
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use assert_fs::{fixture::PathChild, TempDir};
    use cid::multihash::MultihashDigest;
    use cid::Cid;
    use local_storage::provider::SqliteStorageProvider;

    struct TestHarness {
        _storage: Rc<Storage>,
        receiver: Receiver,
        _db_dir: TempDir,
    }

    impl TestHarness {
        pub fn new() -> Self {
            let db_dir = TempDir::new().unwrap();
            let db_path = db_dir.child("storage.db");
            let provider = SqliteStorageProvider::new(db_path.path().to_str().unwrap()).unwrap();
            provider.setup().unwrap();
            let _storage = Rc::new(Storage::new(Box::new(provider)));
            let receiver = Receiver::new(Rc::clone(&_storage));
            return TestHarness {
                _storage,
                receiver,
                _db_dir: db_dir,
            };
        }
    }

    #[tokio::test]
    pub async fn test_receive_block_msg() {
        let mut harness = TestHarness::new();
        let data = b"1871217171".to_vec();
        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(&data));

        let block_msg = DataProtocol::Block(TransmissionBlock {
            cid: cid.to_bytes(),
            data,
            links: vec![],
        });

        let res = harness.receiver.handle_transmission_msg(block_msg).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    pub async fn test_receive_block_msg_twice() {
        let mut harness = TestHarness::new();
        let data = b"18712552224417171".to_vec();
        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(&data));

        let block_msg = DataProtocol::Block(TransmissionBlock {
            cid: cid.to_bytes(),
            data,
            links: vec![],
        });

        let res = harness
            .receiver
            .handle_transmission_msg(block_msg.clone())
            .await;
        assert!(res.is_ok());

        let res = harness.receiver.handle_transmission_msg(block_msg).await;
        assert!(res.is_ok());
    }
}
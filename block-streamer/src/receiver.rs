use anyhow::Result;
use block_ship::{chunking::chunks_to_path, types::BlockWrapper, types::CID_MARKER_LEN};
use cid::Cid;
use iroh_unixfs::Block;
use messages::{TransmissionChunk, TransmissionMessage};
use std::{collections::BTreeMap, path::PathBuf};
use tracing::info;

pub struct Receiver {
    // Map of cid to blocks
    pub blocks: BTreeMap<Cid, Block>,
    // Map of cid to root blocks
    pub roots: BTreeMap<Cid, Block>,
    // Map of cid_marker to cid
    pub cids_to_build: BTreeMap<Vec<u8>, Vec<u8>>,
    // Map of cid_marker to [chunk]
    pub cid_chunks: BTreeMap<Vec<u8>, Vec<TransmissionChunk>>,
}

impl Receiver {
    pub fn new() -> Receiver {
        Receiver {
            blocks: BTreeMap::new(),
            roots: BTreeMap::new(),
            cids_to_build: BTreeMap::new(),
            cid_chunks: BTreeMap::new(),
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
                    let block = wrapper.to_block()?;
                    if !block.links().is_empty() {
                        info!(
                            "Found root block {} with links {:?}",
                            &block.cid(),
                            &block.links()
                        );
                        self.roots.insert(*block.cid(), block);
                    } else {
                        info!(
                            "Found child block {} with {} bytes",
                            &block.cid(),
                            block.data().len()
                        );
                        self.blocks.insert(*block.cid(), block.clone());
                    }
                }
            }
        }
        Ok(())
    }

    // Walks the list of root blocks and attempts to assemble the associated tree by
    // checking if all child blocks are present, and if so writing out to a file
    pub async fn attempt_tree_assembly(&self) -> Result<()> {
        // TODO could optimize this to only attempt block assembly for the last seen CID
        for (_, root) in self.roots.iter() {
            if self.verify_block_complete(*root.cid())? {
                info!("Block complete: {}", &root.cid());
                let path = PathBuf::from(root.cid().to_string());
                if chunks_to_path(&path, root, &self.blocks).await? {
                    info!("Assembly success! {}", &path.display());
                }
            }
        }
        Ok(())
    }

    pub fn verify_block_complete(&self, cid: Cid) -> Result<bool> {
        let block = self.roots.get(&cid);
        match block {
            Some(block) => {
                // Check if all child blocks are present
                for c in block.links().iter() {
                    if !self.blocks.contains_key(c) {
                        info!("Missing cid {}, wait for more data", c);
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            None => {
                info!("Failed to find block for {}", cid);
                Ok(false)
            }
        }
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
    use block_ship::types::{BlockPayload, BlockWrapper};
    use cid::multihash::{Code, MultihashDigest};
    use cid::Cid;
    use futures::TryStreamExt;
    use iroh_unixfs::builder::{File, FileBuilder};

    use super::*;

    #[test]
    pub fn test_receive_chunk() {
        let mut r = Receiver::new();

        let chunk = TransmissionChunk {
            cid_marker: vec![],
            chunk_offset: 0,
            data: vec![],
        };
        r.handle_chunk_msg(chunk.clone()).unwrap();
        let entry = r.cid_chunks.first_key_value().unwrap();
        assert_eq!(entry.0, &chunk.cid_marker);
        assert!(entry.1.contains(&chunk));
    }

    #[test]
    pub fn test_receive_cid() {
        let mut r = Receiver::new();

        let cid = b"101010101010101".to_vec();
        r.handle_cid_msg(cid.clone()).unwrap();

        assert_eq!(r.cids_to_build.first_key_value().unwrap().1, &cid);
    }

    #[tokio::test]
    pub async fn test_child_block_assembly() {
        let mut r = Receiver::new();
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
            r.handle_transmission_msg(c).await.unwrap();
        }

        r.attempt_block_assembly().unwrap();
        assert_eq!(r.blocks.len(), 1);
    }

    #[tokio::test]
    pub async fn test_root_block_assembly() {
        let mut r = Receiver::new();
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
            r.handle_transmission_msg(c).await.unwrap();
        }

        r.attempt_block_assembly().unwrap();
        assert_eq!(r.roots.len(), 1);
    }

    // TODO: write tests for handling incomplete blocks

    // TODO: implement support for handling single block files
    #[ignore]
    #[tokio::test]
    pub async fn test_verify_single_block_complete() {
        let mut r = Receiver::new();
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
            let wrapper = BlockWrapper::from_block(b).unwrap();
            let chunks = wrapper.to_chunks().unwrap();
            msgs.extend(chunks);
        }

        for m in msgs {
            r.handle_transmission_msg(m).await.unwrap();
        }
        let (root, _) = r.roots.first_key_value().unwrap();
        assert_eq!(r.verify_block_complete(*root).unwrap(), true);
    }

    #[tokio::test]
    pub async fn test_verify_multi_block_complete() {
        let mut r = Receiver::new();
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
            let wrapper = BlockWrapper::from_block(b).unwrap();
            let chunks = wrapper.to_chunks().unwrap();
            msgs.extend(chunks);
        }

        for m in msgs {
            r.handle_transmission_msg(m).await.unwrap();
        }
        let (root, _) = r.roots.first_key_value().unwrap();
        assert_eq!(r.verify_block_complete(*root).unwrap(), true);
    }
}

use crate::util::verify_dag;
use anyhow::{anyhow, bail, Result};
use bytes::Bytes;
use cid::Cid;
use ipfs_unixfs::Block;
use std::fmt;
use std::str::FromStr;

#[derive(PartialEq, Clone)]
pub struct StoredBlock {
    pub cid: String,
    pub filename: Option<String>,
    pub data: Vec<u8>,
    pub links: Vec<String>,
}

impl StoredBlock {
    pub fn validate(&self) -> Result<()> {
        // For now we'll just piggy back on the validate logic built
        // into unixfs::Block
        let block: Block = self.try_into()?;
        block.validate()
    }
}

impl fmt::Debug for StoredBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let cid_str = Cid::try_from(self.cid.clone())
            .map(|c| c.to_string())
            .unwrap();

        f.debug_struct("StoredBlock")
            .field("cid", &cid_str)
            .field("filename", &self.filename)
            .field("data", &self.data.len())
            .field("links", &self.links.len())
            .finish()
    }
}

impl TryInto<Block> for &StoredBlock {
    type Error = anyhow::Error;

    fn try_into(self) -> std::result::Result<Block, Self::Error> {
        let cid = Cid::from_str(&self.cid)?;
        let links: Result<Vec<Cid>> = self
            .links
            .iter()
            .map(|l| Cid::from_str(l).map_err(|e| anyhow!(e)))
            .collect();
        let data = Bytes::from(self.data.clone());
        Ok(Block::new(cid, data, links?))
    }
}

pub fn validate_dag(stored_blocks: &[StoredBlock]) -> Result<()> {
    if stored_blocks.is_empty() {
        bail!("No blocks found in dag")
    }
    verify_dag(stored_blocks)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use cid::multihash::MultihashDigest;
    use futures::TryStreamExt;
    use ipfs_unixfs::builder::{File, FileBuilder};
    use rand::{thread_rng, RngCore};

    fn generate_stored_blocks(num_blocks: u8) -> Result<Vec<StoredBlock>> {
        const CHUNK_SIZE: u8 = 20;
        let data_size = CHUNK_SIZE * num_blocks;
        let mut data = Vec::<u8>::new();
        data.resize(data_size.into(), 1);
        thread_rng().fill_bytes(&mut data);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let blocks = rt.block_on(async {
            let file: File = FileBuilder::new()
                .content_bytes(data)
                .name("testfile")
                .fixed_chunker(CHUNK_SIZE.into())
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
                filename: None,
            };

            stored_blocks.push(stored);
        });

        Ok(stored_blocks)
    }

    #[test]
    pub fn test_valid_block_no_links() {
        let blocks = generate_stored_blocks(1).unwrap();
        let stored_block = blocks.first().unwrap();

        assert!(stored_block.validate().is_ok());
    }

    #[test]
    pub fn test_invalid_block_no_links() {
        let mut blocks = generate_stored_blocks(1).unwrap();
        let mut stored_block = blocks.pop().unwrap();
        stored_block.data.extend(b"corruption");

        assert_eq!(
            stored_block.validate().unwrap_err().to_string(),
            "Hash of data does not match the CID."
        );
    }

    #[test]
    pub fn test_valid_block_with_links() {
        let blocks = generate_stored_blocks(5).unwrap();
        let stored_block = blocks.last().unwrap();

        assert!(!stored_block.links.is_empty());
        assert!(stored_block.validate().is_ok());
    }

    #[test]
    pub fn test_valid_block_with_invalid_links() {
        let mut blocks = generate_stored_blocks(7).unwrap();
        let stored_block = blocks.last_mut().unwrap();

        stored_block.links.pop();

        assert!(!stored_block.links.is_empty());
        assert_eq!(
            stored_block.validate().unwrap_err().to_string(),
            "links do not match"
        );
    }

    #[test]
    pub fn test_valid_dag_single_block() {
        let blocks = generate_stored_blocks(1).unwrap();

        assert!(validate_dag(&blocks).is_ok());
    }

    #[test]
    pub fn test_valid_dag_multi_blocks() {
        let blocks = generate_stored_blocks(10).unwrap();

        assert!(validate_dag(&blocks).is_ok());
    }

    #[test]
    pub fn test_dag_with_corrupt_block() {
        let mut blocks = generate_stored_blocks(4).unwrap();

        let first = blocks.first_mut().unwrap();
        first.data.extend(b"corruption");

        assert_eq!(
            validate_dag(&blocks).unwrap_err().to_string(),
            "Hash of data does not match the CID."
        );
    }

    #[test]
    pub fn test_dag_missing_block() {
        let mut blocks = generate_stored_blocks(12).unwrap();

        blocks.remove(0);

        assert_eq!(
            validate_dag(&blocks).unwrap_err().to_string(),
            "Links do not match blocks"
        );
    }

    #[test]
    pub fn test_dag_missing_root() {
        let mut blocks = generate_stored_blocks(7).unwrap();

        blocks.pop();

        assert_eq!(
            validate_dag(&blocks).unwrap_err().to_string(),
            "No root found"
        );
    }

    #[test]
    pub fn test_dag_extra_block() {
        let mut blocks = generate_stored_blocks(6).unwrap();

        let data = b"1871217171".to_vec();
        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(&data));

        blocks.insert(
            1,
            StoredBlock {
                cid: cid.to_string(),
                data,
                links: vec![],
                filename: None,
            },
        );

        assert_eq!(
            validate_dag(&blocks).unwrap_err().to_string(),
            "No root found"
        );
    }

    #[test]
    pub fn test_dag_with_wrong_block() {
        let mut blocks = generate_stored_blocks(9).unwrap();

        let data = b"1871217171".to_vec();
        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(&data));

        // Remove a block and insert one which doesn't belong
        blocks.remove(0);
        blocks.push(StoredBlock {
            cid: cid.to_string(),
            data,
            links: vec![],
            filename: None,
        });

        assert_eq!(
            validate_dag(&blocks).unwrap_err().to_string(),
            "Links do not match blocks"
        );
    }

    #[test]
    pub fn test_dag_with_two_roots() {
        let mut blocks = generate_stored_blocks(9).unwrap();

        let other_blocks = generate_stored_blocks(2).unwrap();

        blocks.extend(other_blocks);

        assert_eq!(
            validate_dag(&blocks).unwrap_err().to_string(),
            "No root found"
        );
    }

    #[test]
    pub fn test_dag_no_blocks() {
        assert_eq!(
            validate_dag(&[]).unwrap_err().to_string(),
            "No blocks found in dag"
        );
    }
}

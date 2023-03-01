use anyhow::{bail, Result};
use bytes::Bytes;
use cid::Cid;
use iroh_unixfs::Block;
use std::collections::HashSet;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub struct StoredBlock {
    pub cid: String,
    pub data: Vec<u8>,
    pub links: Vec<String>,
}

impl StoredBlock {
    pub fn validate(&self) -> Result<()> {
        // For now we'll just piggy back on the validate logic built
        // into n0/beetle:unixfs::Block
        let block: Block = self.try_into()?;
        block.validate()
    }
}

impl TryInto<Block> for &StoredBlock {
    type Error = anyhow::Error;

    fn try_into(self) -> std::result::Result<Block, Self::Error> {
        let cid = Cid::from_str(&self.cid)?;
        let links = self
            .links
            .iter()
            .map(|l| Cid::from_str(l).unwrap())
            .collect::<Vec<Cid>>();
        let data = Bytes::from(self.data.clone());
        Ok(Block::new(cid, data, links))
    }
}

pub fn validate_dag(stored_blocks: Vec<StoredBlock>) -> Result<()> {
    if stored_blocks.is_empty() {
        bail!("No blocks found in dag")
    }
    let blocks = stored_blocks
        .iter()
        .map(|b| b.try_into().unwrap())
        .collect::<Vec<Block>>();
    // First check if all blocks are valid
    for block in blocks.iter() {
        block.validate()?;
    }
    // If there is only one block, and it is valid, then assume it's valid on its own
    if blocks.len() == 1 {
        return Ok(());
    }
    // Otherwise find the root
    let root = blocks.iter().find(|b| !b.links().is_empty());
    if let Some(root) = root {
        // The block's validate function already tells us if the CID's hash matches the root links
        // So now we just need compare the deduplicated sets of CIDs from the child blocks and the root links
        let child_cids = blocks
            .iter()
            .filter(|b| b.links().is_empty())
            .map(|b| b.cid().to_string())
            .collect::<HashSet<String>>();
        let linked_cids = root
            .links()
            .iter()
            .map(|l| l.to_string())
            .collect::<HashSet<String>>();
        if child_cids != linked_cids {
            bail!("Links do not match blocks");
        }
    } else {
        bail!("No root found")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use cid::multihash::MultihashDigest;
    use futures::TryStreamExt;
    use iroh_unixfs::builder::{File, FileBuilder};
    use rand::{thread_rng, RngCore};

    async fn generate_stored_blocks(num_blocks: u8) -> Result<Vec<StoredBlock>> {
        const CHUNK_SIZE: u8 = 20;
        let data_size = CHUNK_SIZE * num_blocks;
        let mut data = Vec::<u8>::new();
        data.resize(data_size.into(), 1);
        thread_rng().fill_bytes(&mut data);

        let file: File = FileBuilder::new()
            .content_bytes(data)
            .name("testfile")
            .fixed_chunker(CHUNK_SIZE.into())
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
    pub async fn test_valid_block_no_links() {
        let blocks = generate_stored_blocks(1).await.unwrap();
        let stored_block = blocks.first().unwrap();

        assert!(stored_block.validate().is_ok());
    }

    #[tokio::test]
    pub async fn test_invalid_block_no_links() {
        let mut blocks = generate_stored_blocks(1).await.unwrap();
        let mut stored_block = blocks.pop().unwrap();
        stored_block.data.extend(b"corruption");

        assert_eq!(
            stored_block.validate().unwrap_err().to_string(),
            "Hash of data does not match the CID."
        );
    }

    #[tokio::test]
    pub async fn test_valid_block_with_links() {
        let blocks = generate_stored_blocks(5).await.unwrap();
        let stored_block = blocks.last().unwrap();

        assert!(stored_block.links.len() > 0);
        assert!(stored_block.validate().is_ok());
    }

    #[tokio::test]
    pub async fn test_valid_block_with_invalid_links() {
        let mut blocks = generate_stored_blocks(7).await.unwrap();
        let stored_block = blocks.last_mut().unwrap();

        stored_block.links.pop();

        assert!(stored_block.links.len() > 0);
        assert_eq!(
            stored_block.validate().unwrap_err().to_string(),
            "links do not match"
        );
    }

    #[tokio::test]
    pub async fn test_valid_dag_single_block() {
        let blocks = generate_stored_blocks(1).await.unwrap();

        assert!(validate_dag(blocks).is_ok());
    }

    #[tokio::test]
    pub async fn test_valid_dag_multi_blocks() {
        let blocks = generate_stored_blocks(10).await.unwrap();

        assert!(validate_dag(blocks).is_ok());
    }

    #[tokio::test]
    pub async fn test_dag_with_corrupt_block() {
        let mut blocks = generate_stored_blocks(4).await.unwrap();

        let first = blocks.first_mut().unwrap();
        first.data.extend(b"corruption");

        assert_eq!(
            validate_dag(blocks).unwrap_err().to_string(),
            "Hash of data does not match the CID."
        );
    }

    #[tokio::test]
    pub async fn test_dag_missing_block() {
        let mut blocks = generate_stored_blocks(12).await.unwrap();

        blocks.remove(0);

        assert_eq!(
            validate_dag(blocks).unwrap_err().to_string(),
            "Links do not match blocks"
        );
    }

    #[tokio::test]
    pub async fn test_dag_missing_root() {
        let mut blocks = generate_stored_blocks(7).await.unwrap();

        blocks.pop();

        assert_eq!(
            validate_dag(blocks).unwrap_err().to_string(),
            "No root found"
        );
    }

    #[tokio::test]
    pub async fn test_dag_extra_block() {
        let mut blocks = generate_stored_blocks(6).await.unwrap();

        let data = b"1871217171".to_vec();
        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(&data));

        blocks.push(StoredBlock {
            cid: cid.to_string(),
            data,
            links: vec![],
        });

        assert_eq!(
            validate_dag(blocks).unwrap_err().to_string(),
            "Links do not match blocks"
        );
    }

    #[tokio::test]
    pub async fn test_dag_with_wrong_block() {
        let mut blocks = generate_stored_blocks(9).await.unwrap();

        let data = b"1871217171".to_vec();
        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(&data));

        // Remove a block and insert one which doesn't belong
        blocks.remove(0);
        blocks.push(StoredBlock {
            cid: cid.to_string(),
            data,
            links: vec![],
        });

        assert_eq!(
            validate_dag(blocks).unwrap_err().to_string(),
            "Links do not match blocks"
        );
    }

    #[tokio::test]
    pub async fn test_dag_no_blocks() {
        assert_eq!(
            validate_dag(vec![]).unwrap_err().to_string(),
            "No blocks found in dag"
        );
    }
}

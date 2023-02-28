use anyhow::Result;
use bytes::Bytes;
use cid::Cid;
use iroh_unixfs::Block;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub struct StoredBlock {
    pub cid: String,
    pub data: Vec<u8>,
    pub links: Vec<String>,
}

impl StoredBlock {
    pub fn validate(&self) -> Result<()> {
        let real_cid = Cid::from_str(&self.cid)?;
        let real_links = self
            .links
            .iter()
            .map(|l| Cid::from_str(l).unwrap())
            .collect::<Vec<Cid>>();
        let real_data = Bytes::from(self.data.clone());
        // For now we'll just piggy back on the validate logic built
        // into n0/beetle:unixfs::Block
        let block = Block::new(real_cid, real_data, real_links);
        block.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::anyhow;
    use futures::TryStreamExt;
    use iroh_unixfs::builder::{File, FileBuilder};
    use libipld::error::InvalidMultihash;

    async fn generate_stored_blocks(data: Vec<u8>) -> Result<Vec<StoredBlock>> {
        let file: File = FileBuilder::new()
            .content_bytes(data)
            .name("testfile")
            .fixed_chunker(20)
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
        let blocks = generate_stored_blocks(vec![0; 10]).await.unwrap();
        let stored_block = blocks.first().unwrap();

        assert!(stored_block.validate().is_ok());
    }

    #[tokio::test]
    pub async fn test_invalid_block_no_links() {
        let mut blocks = generate_stored_blocks(vec![0; 10]).await.unwrap();
        let mut stored_block = blocks.pop().unwrap();
        stored_block.data.extend(b"corruption");

        assert_eq!(
            stored_block.validate().unwrap_err().to_string(),
            anyhow!(InvalidMultihash(stored_block.data)).to_string(),
        );
    }

    #[tokio::test]
    pub async fn test_valid_block_with_links() {
        let blocks = generate_stored_blocks(vec![15; 50]).await.unwrap();
        let stored_block = blocks.last().unwrap();

        assert!(stored_block.links.len() > 0);
        assert!(stored_block.validate().is_ok());
    }

    #[tokio::test]
    pub async fn test_valid_block_with_invalid_links() {
        let mut blocks = generate_stored_blocks(vec![15; 50]).await.unwrap();
        let stored_block = blocks.last_mut().unwrap();

        stored_block.links.pop();

        assert!(stored_block.links.len() > 0);
        assert_eq!(
            stored_block.validate().unwrap_err().to_string(),
            "links do not match"
        );
    }
}

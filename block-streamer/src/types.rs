use anyhow::Result;
use bytes::Bytes;
use cid::Cid;
use iroh_resolver::resolver::Block;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct DataBlob {
    pub cid: Vec<u8>,
    pub data: Vec<u8>,
    pub links: Vec<Vec<u8>>,
}

impl DataBlob {
    pub fn as_block(&self) -> Result<Block> {
        let mut links = vec![];
        for l in &self.links {
            links.push(Cid::try_from(l.clone())?);
        }
        Ok(Block::new(
            Cid::try_from(self.cid.clone())?,
            Bytes::from(self.data.clone()),
            links,
        ))
    }

    pub fn from_block(block: Block) -> Result<Self> {
        let mut links = vec![];
        for l in block.links() {
            links.push(l.to_bytes());
        }
        Ok(DataBlob {
            cid: block.cid().to_bytes(),
            data: block.data().to_vec(),
            links,
        })
    }
}

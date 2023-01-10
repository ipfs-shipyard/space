use anyhow::Result;
use bytes::Bytes;
use cid::Cid;
use iroh_unixfs::Block;
use parity_scale_codec_derive::{Decode, Encode};

#[derive(Clone, Debug, Decode, Encode)]
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

        // Right now we're ignoring the data attached to the root nodes
        // because the current assembly method doesn't require it
        // and it saves a decent amount of payload weight
        let data = if !links.is_empty() {
            vec![]
        } else {
            block.data().to_vec()
        };
        Ok(DataBlob {
            cid: block.cid().to_bytes(),
            data,
            links,
        })
    }
}

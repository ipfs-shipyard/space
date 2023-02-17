use anyhow::{anyhow, Result};
use bytes::Bytes;
use cid::Cid;
use iroh_unixfs::Block;
use local_storage::storage::StoredBlock;
use messages::{TransmissionChunk, TransmissionMessage};
use parity_scale_codec::{Decode, Encode};
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use std::convert::{From, TryFrom};
use tracing::error;

const CHUNK_SIZE: usize = 40;

// TODO: Create a function to more cleanly create this marker
// and pull it from *just* the cid hash digest
pub const CID_MARKER_LEN: usize = 10;

#[derive(Debug, Eq, PartialEq)]
pub struct BlockWrapper {
    pub cid: Vec<u8>,
    pub payload: BlockPayload,
}

#[derive(Debug, Eq, ParityDecode, ParityEncode, PartialEq)]
pub struct BlockPayload {
    pub data: Vec<u8>,
    pub links: Vec<Vec<u8>>,
}

impl From<Block> for BlockWrapper {
    fn from(block: Block) -> Self {
        let links = block
            .links()
            .iter()
            .map(|l| l.to_bytes())
            .collect::<Vec<Vec<u8>>>();

        // Right now we're ignoring the data attached to the root nodes
        // because the current assembly method doesn't require it
        // and it saves a decent amount of payload weight
        let data = if !links.is_empty() {
            vec![]
        } else {
            block.data().to_vec()
        };

        BlockWrapper {
            cid: block.cid().to_bytes(),
            payload: BlockPayload { data, links },
        }
    }
}

impl TryFrom<BlockWrapper> for Block {
    type Error = anyhow::Error;

    fn try_from(value: BlockWrapper) -> std::result::Result<Self, Self::Error> {
        let links = value
            .payload
            .links
            .iter()
            .filter_map(|l| match Cid::try_from(l.clone()) {
                Ok(cid) => Some(cid),
                Err(e) => {
                    error!("Failed to parse CID from {l:?}: {e}");
                    None
                }
            })
            .collect::<Vec<Cid>>();
        Ok(Block::new(
            Cid::try_from(value.cid.clone())?,
            Bytes::from(value.payload.data),
            links,
        ))
    }
}

impl TryFrom<&StoredBlock> for BlockWrapper {
    type Error = anyhow::Error;

    fn try_from(block: &StoredBlock) -> std::result::Result<Self, Self::Error> {
        let links = block
            .links
            .iter()
            .filter_map(|l| match Cid::try_from(l.to_string()) {
                Ok(cid) => Some(cid.to_bytes()),
                Err(e) => {
                    error!("Failed to parse CID {l}: {e}");
                    None
                }
            })
            .collect::<Vec<Vec<u8>>>();

        // Right now we're ignoring the data attached to the root nodes
        // because the current assembly method doesn't require it
        // and it saves a decent amount of payload weight
        let data = if !links.is_empty() {
            vec![]
        } else {
            block.data.to_vec()
        };

        Ok(BlockWrapper {
            cid: Cid::try_from(block.cid.to_string())?.to_bytes(),
            payload: BlockPayload { data, links },
        })
    }
}

impl TryFrom<BlockWrapper> for StoredBlock {
    type Error = anyhow::Error;

    fn try_from(value: BlockWrapper) -> std::result::Result<Self, Self::Error> {
        let links = value
            .payload
            .links
            .iter()
            .filter_map(|l| match Cid::try_from(l.to_owned()) {
                Ok(cid) => Some(cid.to_string()),
                Err(e) => {
                    error!("Failed to parse CID from {l:?}: {e}");
                    None
                }
            })
            .collect::<Vec<String>>();
        Ok(StoredBlock {
            cid: Cid::try_from(value.cid.clone())?.to_string(),
            data: value.payload.data,
            links,
        })
    }
}

impl BlockWrapper {
    pub fn to_chunks(&self) -> Result<Vec<TransmissionMessage>> {
        let cid_marker = &self.cid[..CID_MARKER_LEN];
        let mut chunks = vec![];

        chunks.push(TransmissionMessage::Cid(self.cid.clone()));

        let encoded_payload = self.payload.encode();
        for (offset, chunk) in (0_u16..).zip(encoded_payload.chunks(CHUNK_SIZE)) {
            chunks.push(TransmissionMessage::Chunk(TransmissionChunk {
                cid_marker: cid_marker.to_vec(),
                chunk_offset: offset,
                data: chunk.to_vec(),
            }));
        }

        Ok(chunks)
    }

    // TODO: This should probably verify the hash against the data
    pub fn from_chunks(cid: &[u8], messages: &[TransmissionChunk]) -> Result<Self> {
        let blob: Vec<u8> = messages.iter().flat_map(|c| c.data.clone()).collect();
        if let Ok(payload) = BlockPayload::decode(&mut blob.as_slice()) {
            return Ok(BlockWrapper {
                cid: cid.to_vec(),
                payload,
            });
        }
        Err(anyhow!("Failed to find payload"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cid::multihash::{Code, MultihashDigest};
    use cid::Cid;

    #[test]
    pub fn test_block_to_and_from_wrapper() {
        let h = Code::Sha2_256.digest(b"test this");
        let cid = Cid::new_v1(0x55, h);

        let block = Block::new(cid, vec![].into(), vec![cid.clone()]);
        let wrapper = BlockWrapper::try_from(block.clone()).unwrap();
        let block_again: Block = wrapper.try_into().unwrap();
        assert_eq!(block, block_again);
    }

    #[test]
    pub fn test_stored_block_to_and_from_wrapper() {
        let h = Code::Sha2_256.digest(b"test this");
        let cid = Cid::new_v1(0x55, h);

        let stored_block = StoredBlock {
            cid: cid.to_string(),
            data: vec![],
            links: vec![cid.to_string()],
        };

        let wrapper = BlockWrapper::try_from(&stored_block).unwrap();

        let stored_block_again: StoredBlock = wrapper.try_into().unwrap();
        assert_eq!(stored_block, stored_block_again);
    }

    #[test]
    pub fn test_chunk_and_rebuild_block() {
        let cid = b"12345678901230123".to_vec();
        let wrapper = BlockWrapper {
            cid: cid.clone(),
            payload: BlockPayload {
                data: vec![4, 5, 6],
                links: vec![vec![1], vec![2]],
            },
        };

        let messages = wrapper.to_chunks().unwrap();
        let chunks: Vec<TransmissionChunk> = messages
            .iter()
            .filter_map(|mes| match mes {
                TransmissionMessage::Chunk(chunk) => Some(chunk.clone()),
                TransmissionMessage::Cid(_) => None,
                TransmissionMessage::Block(_) => None,
            })
            .collect();
        let rebuilt = BlockWrapper::from_chunks(&cid, &chunks).unwrap();
        assert_eq!(wrapper, rebuilt);
    }
}

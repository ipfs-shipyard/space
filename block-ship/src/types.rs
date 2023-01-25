use anyhow::{anyhow, Result};
use bytes::Bytes;
use cid::Cid;
use iroh_unixfs::Block;
use parity_scale_codec::{Decode, Encode};
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};

const CHUNK_SIZE: usize = 40;

#[derive(Clone, Debug, ParityDecode, ParityEncode)]
pub struct TransmissionChunk {
    pub cid_marker: Vec<u8>,
    pub chunk_offset: u16,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, ParityDecode, ParityEncode)]
pub enum TransmissionMessage {
    Cid(Vec<u8>),
    Chunk(TransmissionChunk),
}

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

impl BlockWrapper {
    pub fn to_block(&self) -> Result<Block> {
        let mut links = vec![];
        for l in &self.payload.links {
            links.push(Cid::try_from(l.clone())?);
        }
        Ok(Block::new(
            Cid::try_from(self.cid.clone())?,
            Bytes::from(self.payload.data.clone()),
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

        Ok(BlockWrapper {
            cid: block.cid().to_bytes(),
            payload: BlockPayload { data, links },
        })
    }

    pub fn to_chunks(&self) -> Result<Vec<TransmissionMessage>> {
        let cid_marker = &self.cid[..4];
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
                cid: cid.to_owned(),
                payload,
            });
        }
        Err(anyhow!("Failed to find payload"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_chunk_and_rebuild_block() {
        let cid = vec![1, 2, 4, 5, 6];
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
            })
            .collect();
        dbg!(&chunks);
        let rebuilt = BlockWrapper::from_chunks(&cid, &chunks).unwrap();
        assert_eq!(wrapper, rebuilt);
    }
}

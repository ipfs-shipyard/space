use crate::Message;

use anyhow::{bail, Result};
use parity_scale_codec::{Decode, Encode};
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use std::collections::BTreeMap;

pub trait MessageChunker {
    fn chunk(&self, message: Message) -> Result<Vec<Vec<u8>>>;
    fn unchunk(&mut self, data: &Vec<u8>) -> Result<Message>;
}

#[derive(Clone, Debug, ParityDecode, ParityEncode)]
pub struct SimpleChunk {
    // Random 16-bit ID used to identify the message this chunk belongs to
    pub message_id: u16,
    // Sequence number indicates the order of reassembly
    pub sequence_number: u16,
    // Final chunk flag indicates the last chunk in sequence
    pub final_chunk: bool,
    // Data payload
    pub data: Vec<u8>,
}

// This const is derived from the size of the above struct when encoded with SCALE
// and verified using a test below. It appears to consistently be seven,
// except when data is fairly small
const CHUNK_OVERHEAD: u16 = 7;

pub struct SimpleChunker {
    // Max message size
    mtu: u16,
    // Map of message IDs to maps of sequence numbers and message chunks
    // { message_id: { sequence_id: data }}
    recv_buffer: BTreeMap<u16, BTreeMap<u16, SimpleChunk>>,
    // Last received message_id to optimize reassembly searching
    last_recv_msg_id: u16,
}

impl SimpleChunker {
    pub fn new(mtu: u16) -> Self {
        Self {
            mtu,
            recv_buffer: BTreeMap::<u16, BTreeMap<u16, SimpleChunk>>::new(),
            last_recv_msg_id: 0,
        }
    }

    fn recv_chunk(&mut self, chunk: &SimpleChunk) -> Result<()> {
        if let Some(msg_map) = self.recv_buffer.get_mut(&chunk.message_id) {
            println!("new chunk for existing message, add to map");
            msg_map.insert(chunk.sequence_number, chunk.clone());
        } else {
            println!("new message new chunk, create maps");
            let mut msg_map: BTreeMap<u16, SimpleChunk> = BTreeMap::new();
            msg_map.insert(chunk.sequence_number, chunk.clone());
            self.recv_buffer.insert(chunk.message_id, msg_map);
        }
        self.last_recv_msg_id = chunk.message_id;

        Ok(())
    }

    fn attempt_msg_assembly(&mut self) -> Result<Message> {
        println!("attempt msg assembly {}", self.last_recv_msg_id);
        if let Some(msg_map) = self.recv_buffer.get(&self.last_recv_msg_id) {
            println!("found a msg map");
            let chunks = msg_map.clone().into_values().collect::<Vec<SimpleChunk>>();
            println!("chunks: {:?}", chunks);
            // First see if we have a final chunk
            if let Some(final_chunk) = chunks.iter().find(|chunk| chunk.final_chunk) {
                // Then see if the number of chunks matches the max sequence number
                if usize::from(final_chunk.sequence_number) == (chunks.len() - 1) {
                    // Now actually attempt to assemble the message
                    return SimpleChunker::msg_unchunk(&chunks);
                }
            }
        }

        bail!("No message found")
    }

    fn msg_unchunk(data: &Vec<SimpleChunk>) -> Result<Message> {
        let mut all_data = vec![];
        data.iter().for_each(|c| all_data.extend(c.data.clone()));
        let mut databuf = &all_data[..all_data.len()];
        Ok(Message::decode(&mut databuf)?)
    }
}

impl MessageChunker for SimpleChunker {
    fn chunk(&self, message: Message) -> Result<Vec<Vec<u8>>> {
        let msg_id = rand::random::<u16>();
        let mut seq_num = 0;
        // Convert message into raw bytes
        let message_bytes = message.to_bytes();
        // Break bytes up into mtu-sized simple chunks
        let mut chunks = message_bytes
            .chunks(usize::from(self.mtu - CHUNK_OVERHEAD))
            .map(|data| {
                let chunk = SimpleChunk {
                    message_id: msg_id,
                    sequence_number: seq_num,
                    data: data.to_vec(),
                    final_chunk: false,
                };
                seq_num = seq_num + 1;
                chunk
            })
            .collect::<Vec<SimpleChunk>>();
        // Set final to true in last chunk
        let mut last_chunk = chunks.last_mut().unwrap();
        (*last_chunk).final_chunk = true;
        // Encode all the chunks
        Ok(chunks.iter().map(|c| c.encode()).collect::<Vec<Vec<u8>>>())
    }

    fn unchunk(&mut self, data: &Vec<u8>) -> Result<Message> {
        let mut databuf = &data[..data.len()];
        match SimpleChunk::decode(&mut databuf) {
            Ok(chunk) => {
                println!("decode chunk");
                self.recv_chunk(&chunk)?;
                return Ok(self.attempt_msg_assembly()?);
            }
            Err(e) => bail!("Failed to decode chunk {e:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ApplicationAPI;
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    #[test]
    pub fn test_chunk_overhead_against_const() {
        let data_sizes = [600, 2500, 5000, 10240];
        for size in data_sizes {
            let chunk = SimpleChunk {
                message_id: size,
                sequence_number: size,
                final_chunk: true,
                data: vec![80; usize::from(size)],
            };
            let chunk_encoded_size = chunk.encoded_size();
            assert_eq!(
                chunk_encoded_size - usize::from(size),
                usize::from(CHUNK_OVERHEAD)
            );
        }
    }

    #[test]
    pub fn test_chunking_under_various_mtus() {
        let mtu_list = [60, 250, 500, 1024];

        for mtu in mtu_list {
            let chunker = SimpleChunker::new(mtu);
            let msg = Message::ApplicationAPI(ApplicationAPI::MissingDagBlocks {
                blocks: vec!["data".to_string(); 10240],
            });

            let chunks = chunker.chunk(msg).unwrap();

            // We don't check length on the final chunk because it isn't always full
            for c in &chunks[..chunks.len() - 1] {
                // Length of each chunk should be less than or equal to MTU
                assert!(c.len() <= usize::from(mtu));
                // And should only be 2 bytes lower than MTU
                assert!(c.len() >= usize::from(mtu - 2));
            }
        }
    }

    // Testing scenario where a single message is broken into a single chunk
    #[test]
    pub fn test_chunk_and_unchunk_single_message_single_chunk() {
        let msg = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids: vec![] });
        let mut chunker = SimpleChunker::new(60);
        let chunks = chunker.chunk(msg.clone()).unwrap();

        assert_eq!(chunks.len(), 1);

        let unchunked_message = chunker.unchunk(chunks.first().unwrap()).unwrap();
        assert_eq!(msg, unchunked_message);
    }

    // Testing scenario where a single message is broken into multiple chunks in sequential order
    #[test]
    pub fn test_chunk_and_unchunk_single_message_multi_chunk_sequential() {
        let msg = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a CID".to_string(); 10],
        });
        let mut chunker = SimpleChunker::new(60);
        let mut chunks = chunker.chunk(msg.clone()).unwrap();

        assert_eq!(chunks.len(), 4);

        let last_chunk = chunks.pop().unwrap();

        for chunk in chunks {
            let unchunk = chunker.unchunk(&chunk);
            assert!(unchunk.is_err());
        }
        let unchunked_msg = chunker.unchunk(&last_chunk).unwrap();
        assert_eq!(unchunked_msg, msg);
    }

    // Testing scenario where a single message is broken into multiple chunks in random order
    #[test]
    pub fn test_chunk_and_unchunk_single_message_multi_chunk_random() {
        let msg = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a CID".to_string(); 10],
        });
        let mut chunker = SimpleChunker::new(60);
        let mut chunks = chunker.chunk(msg.clone()).unwrap();

        assert_eq!(chunks.len(), 4);

        // This randomly shuffles the order of the blocks (prior to chunking)
        // in order to exercise reassembly on the receiver side.
        chunks.shuffle(&mut thread_rng());

        let last_chunk = chunks.pop().unwrap();

        for chunk in chunks {
            let unchunk = chunker.unchunk(&chunk);
            assert!(unchunk.is_err());
        }
        let unchunked_msg = chunker.unchunk(&last_chunk).unwrap();
        assert_eq!(unchunked_msg, msg);
    }

    // Testing scenario where two messages are broken into single chunks in sequential order
    #[test]
    pub fn test_chunk_and_unchunk_two_message_single_chunk_sequential() {
        let msg_one = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello I am a CID".to_string()],
        });
        let msg_two = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello I am a different CID".to_string()],
        });
        let mut chunker = SimpleChunker::new(60);
        let msg_one_chunks = chunker.chunk(msg_one.clone()).unwrap();
        let msg_two_chunks = chunker.chunk(msg_two.clone()).unwrap();

        let unchunked_message = chunker.unchunk(msg_one_chunks.first().unwrap()).unwrap();
        assert_eq!(msg_one, unchunked_message);

        let unchunked_message = chunker.unchunk(msg_two_chunks.first().unwrap()).unwrap();
        assert_eq!(msg_two, unchunked_message);
    }

    // Testing scenario where two messages are broken into multiple chunks in sequential order
    #[test]
    pub fn test_chunk_and_unchunk_two_message_multi_chunk_sequential() {
        let msg_one = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a CID".to_string(); 10],
        });
        let msg_two = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a different CID".to_string(); 10],
        });
        let mut chunker = SimpleChunker::new(60);
        let msg_one_chunks = chunker.chunk(msg_one.clone()).unwrap();
        let msg_two_chunks = chunker.chunk(msg_two.clone()).unwrap();

        let mut chunks = vec![];
        chunks.extend(msg_one_chunks);
        chunks.extend(msg_two_chunks);

        let mut found_msgs = 0;
        for chunk in chunks {
            match chunker.unchunk(&chunk) {
                Ok(msg) => {
                    assert!([&msg_one, &msg_two].contains(&&msg));
                    found_msgs = found_msgs + 1;
                }
                Err(_) => {}
            }
        }
        assert_eq!(found_msgs, 2);
    }

    // Testing scenario where two messages are broken into multiple chunks in random order
    #[test]
    pub fn test_chunk_and_unchunk_two_message_multi_chunk_random() {
        let msg_one = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a CID".to_string(); 10],
        });
        let msg_two = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a different CID".to_string(); 10],
        });
        let mut chunker = SimpleChunker::new(60);
        let msg_one_chunks = chunker.chunk(msg_one.clone()).unwrap();
        let msg_two_chunks = chunker.chunk(msg_two.clone()).unwrap();

        let mut chunks = vec![];
        chunks.extend(msg_one_chunks);
        chunks.extend(msg_two_chunks);

        chunks.shuffle(&mut thread_rng());

        let mut found_msgs = 0;
        for chunk in chunks {
            match chunker.unchunk(&chunk) {
                Ok(msg) => {
                    assert!([&msg_one, &msg_two].contains(&&msg));
                    found_msgs = found_msgs + 1;
                }
                Err(_) => {}
            }
        }
        assert_eq!(found_msgs, 2);
    }

    // Testing scenario where multiple messages are broken into multiple chunks in random order
    #[test]
    pub fn test_chunk_and_unchunk_multi_message_multi_chunk_random() {
        let test_seeds = [
            ("hello I am a CID", 20),
            ("hello I am not a CID", 45),
            ("what is a CID??", 2),
            ("Is this a CID!?", 200),
        ];
        let mut msgs = vec![];
        for (cid, len) in test_seeds {
            msgs.push(Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
                cids: vec![cid.to_string(); len],
            }));
        }

        let mut chunker = SimpleChunker::new(60);

        let mut chunks = vec![];
        for msg in &msgs {
            chunks.extend(chunker.chunk(msg.clone()).unwrap());
        }

        chunks.shuffle(&mut thread_rng());

        let mut found_msgs = 0;
        for chunk in chunks {
            match chunker.unchunk(&chunk) {
                Ok(msg) => {
                    assert!(msgs.contains(&msg));
                    found_msgs = found_msgs + 1;
                }
                Err(_) => {}
            }
        }
        assert_eq!(found_msgs, msgs.len());
    }
}

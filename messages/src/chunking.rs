use crate::message::{Message, MessageContainer};
use crate::DataProtocol;

use anyhow::{bail, Result};
use mini_moka::sync::Cache;
use parity_scale_codec::{Decode, Encode};
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use std::collections::BTreeMap;
use std::time::Duration;

pub trait MessageChunker {
    fn get_prev_sent_chunks(&self, chunk_map: Vec<(u16, u16)>) -> Result<Vec<Vec<u8>>>;
    fn chunk(&self, message: Message) -> Result<Vec<Vec<u8>>>;
    fn unchunk(&mut self, data: &[u8]) -> Result<Option<UnchunkResult>>;
    fn find_missing_chunks(&self) -> Result<Vec<Vec<u8>>>;
}

#[derive(Clone, Debug, ParityDecode, ParityEncode, PartialEq)]
pub enum UnchunkResult {
    Message(Message),
    Missing(MissingChunks),
}

#[derive(Clone, Debug, ParityDecode, ParityEncode)]
pub enum SimpleMsg {
    Chunk(SimpleChunk),
    Missing(MissingChunks),
}

#[derive(Clone, Debug, ParityDecode, ParityEncode, PartialEq)]
pub struct MissingChunks(pub Vec<(u16, u16)>);

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
    recv_cache: Cache<u16, BTreeMap<u16, SimpleChunk>>,
    // Last received message_id to optimize reassembly searching
    last_recv_msg_id: u16,
    // Cache of sent messages
    sent_cache: Cache<u16, Vec<SimpleChunk>>,
}

impl SimpleChunker {
    pub fn new(mtu: u16) -> Self {
        let sent_cache = Cache::builder()
            .initial_capacity(500)
            .time_to_idle(Duration::from_secs(30))
            .build();
        let recv_cache = Cache::builder()
            .initial_capacity(500)
            .time_to_idle(Duration::from_secs(30))
            .build();
        Self {
            mtu,
            recv_cache,
            last_recv_msg_id: 0,
            sent_cache,
        }
    }

    fn recv_chunk(&mut self, chunk: SimpleChunk) -> Result<()> {
        let msg_id = chunk.message_id;
        self.last_recv_msg_id = msg_id;
        if let Some(mut msg_map) = self.recv_cache.get(&msg_id) {
            msg_map.insert(chunk.sequence_number, chunk);
            self.recv_cache.insert(msg_id, msg_map);
        } else {
            let mut msg_map: BTreeMap<u16, SimpleChunk> = BTreeMap::new();
            msg_map.insert(chunk.sequence_number, chunk);
            self.recv_cache.insert(self.last_recv_msg_id, msg_map);
        }

        Ok(())
    }

    fn attempt_msg_assembly(&mut self) -> Result<Option<UnchunkResult>> {
        // TODO: This needs to be expanded beyond just assembling off the last received message id
        // TODO: Data needs to be removed from the map once the message is assembled correctly
        // TODO: Stale data in the map needs to be cleaned up periodically
        if let Some(msg_map) = self.recv_cache.get(&self.last_recv_msg_id) {
            // The BTreeMap docs tell us that into_values will be an iter sorted by key
            // In this case the key is the sequence_number, so in a complete set of chunks
            // that means the last item in the iter (or now vec) should be the "final chunk"
            let chunks = msg_map.values().collect::<Vec<&SimpleChunk>>();
            // So to verify we have all message chunks...First grab the last chunk in the list
            if let Some(last_chunk) = chunks.last() {
                // Second, check if the last chunk has final_chunk set
                if last_chunk.final_chunk
                // Lastly, check if the final chunk's sequence number matches the number of chunks
                && (usize::from(last_chunk.sequence_number) == (chunks.len() - 1))
                {
                    // If all those checks pass, then we *should* have all the chunks in order
                    // Now we attempt to assemble the message
                    return Ok(Some(UnchunkResult::Message(SimpleChunker::msg_unchunk(
                        &chunks,
                    )?)));
                }
            }
        }

        Ok(None)
    }

    fn msg_unchunk(data: &[&SimpleChunk]) -> Result<Message> {
        let mut all_data = vec![];
        data.iter().for_each(|c| all_data.extend(&c.data));
        let mut databuf = &all_data[..all_data.len()];
        let container = MessageContainer::from_bytes(&mut databuf)?;
        Ok(container.message)
    }

    pub fn find_missing_chunks_map(&self) -> Result<Vec<(u16, u16)>> {
        let mut missing_chunks: Vec<(u16, u16)> = vec![];

        for entry in self.recv_cache.iter() {
            let msg_id = entry.key();
            let chunks = entry.value().values().collect::<Vec<&SimpleChunk>>();
            let mut previous_seq_number = None;
            let mut found_last = false;

            for c in chunks {
                let prev_seq_num = if let Some(prev_seq_num) = previous_seq_number {
                    prev_seq_num + 1
                } else if c.sequence_number > 0 {
                    0
                } else {
                    c.sequence_number
                };
                for missing in (prev_seq_num)..c.sequence_number {
                    missing_chunks.push((*msg_id, missing));
                }
                previous_seq_number = Some(c.sequence_number);
                found_last = c.final_chunk;
            }

            if !found_last {
                missing_chunks.push((*msg_id, previous_seq_number.unwrap() + 1));
            }
        }

        // println!("found missing chunks: {missing_chunks:?}");
        Ok(missing_chunks)
    }
}

impl MessageChunker for SimpleChunker {
    fn chunk(&self, message: Message) -> Result<Vec<Vec<u8>>> {
        let msg_id = rand::random::<u16>();
        let mut seq_num = 0;
        // Create container around message
        let container = MessageContainer::new(message.clone());
        // Convert container into raw bytes
        let message_bytes = container.to_bytes();
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
                seq_num += 1;
                chunk
            })
            .collect::<Vec<SimpleChunk>>();
        // Set final to true in last chunk
        if let Some(mut last_chunk) = chunks.last_mut() {
            last_chunk.final_chunk = true;
        }
        if let Message::DataProtocol(DataProtocol::Block(_)) = message {
            // don't cache these
        } else {
            self.sent_cache.insert(msg_id, chunks.clone());
        }

        // Encode all the chunks
        Ok(chunks
            .iter()
            .map(|c| SimpleMsg::Chunk(c.clone()).encode())
            .collect::<Vec<Vec<u8>>>())
    }

    fn unchunk(&mut self, data: &[u8]) -> Result<Option<UnchunkResult>> {
        let mut databuf = &data[..data.len()];
        match SimpleMsg::decode(&mut databuf) {
            Ok(SimpleMsg::Chunk(chunk)) => {
                self.recv_chunk(chunk)?;
                return Ok(self.attempt_msg_assembly()?);
            }
            Ok(SimpleMsg::Missing(missing)) => {
                return Ok(Some(UnchunkResult::Missing(missing)));
            }
            Err(e) => {
                bail!("Failed to decode: {e:?}");
            }
        }
    }

    fn get_prev_sent_chunks(&self, chunk_map: Vec<(u16, u16)>) -> Result<Vec<Vec<u8>>> {
        let mut found_chunks = vec![];

        for (msg_id, chunk_seq) in chunk_map {
            if let Some(sent_chunks) = self.sent_cache.get(&msg_id) {
                if let Some(found) = sent_chunks
                    .iter()
                    .find(|sc| sc.sequence_number == chunk_seq)
                {
                    found_chunks.push(SimpleMsg::Chunk(found.clone()).encode());
                }
            }
        }

        Ok(found_chunks)
    }

    fn find_missing_chunks(&self) -> Result<Vec<Vec<u8>>> {
        let mut msgs = vec![];
        let missing_map = self.find_missing_chunks_map()?;
        // Overhead is 4 + 2 per missing
        let range_allowed_by_mtu = ((self.mtu - 4) / 4) - 1;
        for chunk in missing_map.chunks(range_allowed_by_mtu.into()) {
            let msg_encode = SimpleMsg::Missing(MissingChunks(chunk.to_vec())).encode();
            msgs.push(msg_encode);
        }
        Ok(msgs)
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
                cid: "notarealcid".to_string(),
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

        if let UnchunkResult::Message(unchunked_message) =
            chunker.unchunk(chunks.first().unwrap()).unwrap().unwrap()
        {
            assert_eq!(msg, unchunked_message);
        } else {
            panic!("Failed to find correct message");
        }
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
            let unchunk = chunker.unchunk(&chunk).unwrap();
            assert!(unchunk.is_none());
        }
        if let UnchunkResult::Message(unchunked_msg) =
            chunker.unchunk(&last_chunk).unwrap().unwrap()
        {
            assert_eq!(unchunked_msg, msg);
        } else {
            panic!("Failed to find correct message");
        }
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
            let unchunk = chunker.unchunk(&chunk).unwrap();
            assert!(unchunk.is_none());
        }
        if let UnchunkResult::Message(unchunked_msg) =
            chunker.unchunk(&last_chunk).unwrap().unwrap()
        {
            assert_eq!(unchunked_msg, msg);
        } else {
            panic!("Failed to find correct message");
        }
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

        if let UnchunkResult::Message(unchunked_message) = chunker
            .unchunk(msg_one_chunks.first().unwrap())
            .unwrap()
            .unwrap()
        {
            assert_eq!(msg_one, unchunked_message);
        } else {
            panic!("Failed to decode message");
        }

        if let UnchunkResult::Message(unchunked_message) = chunker
            .unchunk(msg_two_chunks.first().unwrap())
            .unwrap()
            .unwrap()
        {
            assert_eq!(msg_two, unchunked_message);
        } else {
            panic!("Failed to decode message");
        }
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
                Ok(Some(UnchunkResult::Message(msg))) => {
                    assert!([&msg_one, &msg_two].contains(&&msg));
                    found_msgs += 1;
                }
                Ok(other) => {}
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
                Ok(Some(UnchunkResult::Message(msg))) => {
                    assert!([&msg_one, &msg_two].contains(&&msg));
                    found_msgs += 1;
                }
                Ok(other) => {}
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
                Ok(Some(UnchunkResult::Message(msg))) => {
                    assert!(msgs.contains(&msg));
                    found_msgs += 1;
                }
                Ok(other) => {}
                Err(_) => {}
            }
        }
        assert_eq!(found_msgs, msgs.len());
    }

    // Testing scenario where a chunk is corrupted and verification on assembly fails
    #[test]
    pub fn test_corrupt_data_fails_verification() {
        let msg = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a CID".to_string(); 10],
        });
        let mut chunker = SimpleChunker::new(60);
        let mut chunks = chunker.chunk(msg).unwrap();

        assert_eq!(chunks.len(), 4);

        let mut last_chunk = chunks.pop().unwrap();
        // Adding corruption
        last_chunk[10] = 0x55;

        for chunk in chunks {
            let unchunk = chunker.unchunk(&chunk).unwrap();
            assert!(unchunk.is_none());
        }
        let unchunked_msg = chunker.unchunk(&last_chunk);
        assert!(unchunked_msg.is_err());
    }

    // Testing scenario where single missing chunk is identified
    #[test]
    pub fn test_find_single_missing_chunk() {
        let msg = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a CID".to_string(); 10],
        });
        let mut chunker = SimpleChunker::new(60);
        let mut chunks = chunker.chunk(msg).unwrap();

        assert_eq!(chunks.len(), 4);

        let missing_chunk = chunks.remove(2);
        let mut databuf = &missing_chunk[..missing_chunk.len()];
        let missing_chunk = if let Ok(chunk) = SimpleChunk::decode(&mut databuf) {
            chunk
        } else {
            panic!("decode removed chunk failed");
        };

        for chunk in chunks {
            let unchunk = chunker.unchunk(&chunk).unwrap();
            assert!(unchunk.is_none());
        }

        let missing_chunks = chunker.find_missing_chunks_map().unwrap();
        let mut missing_map = vec![(missing_chunk.message_id, missing_chunk.sequence_number)];

        assert_eq!(missing_chunks, missing_map);
    }

    // Testing scenario where single missing chunk is identified
    #[test]
    pub fn test_find_first_missing_chunk() {
        let msg = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a CID".to_string(); 10],
        });
        let mut chunker = SimpleChunker::new(60);
        let mut chunks = chunker.chunk(msg).unwrap();

        assert_eq!(chunks.len(), 4);

        let missing_chunk = chunks.remove(0);
        let mut databuf = &missing_chunk[..missing_chunk.len()];
        let missing_chunk = if let Ok(chunk) = SimpleChunk::decode(&mut databuf) {
            chunk
        } else {
            panic!("decode removed chunk failed");
        };

        for chunk in chunks {
            let unchunk = chunker.unchunk(&chunk).unwrap();
            assert!(unchunk.is_none());
        }

        let missing_chunks = chunker.find_missing_chunks_map().unwrap();
        let mut missing_map = vec![(missing_chunk.message_id, missing_chunk.sequence_number)];

        assert_eq!(missing_chunks, missing_map);
    }

    // Testing scenario where multiple missing chunks are identified
    #[test]
    pub fn test_find_multiple_missing_chunks() {
        let msg = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a CID".to_string(); 100],
        });
        let mut chunker = SimpleChunker::new(60);
        let mut chunks = chunker.chunk(msg).unwrap();

        let missing_chunk = chunks.remove(2);
        let mut databuf = &missing_chunk[..missing_chunk.len()];
        let missing_chunk = if let Ok(chunk) = SimpleChunk::decode(&mut databuf) {
            chunk
        } else {
            panic!("decode removed chunk failed");
        };

        // Remove a few more chunks for fun
        chunks.remove(15);
        chunks.remove(15);
        chunks.remove(25);

        for chunk in chunks {
            let unchunk = chunker.unchunk(&chunk).unwrap();
            assert!(unchunk.is_none());
        }

        let missing_chunks = chunker.find_missing_chunks_map().unwrap();
        let mut missing_map = vec![
            (missing_chunk.message_id, 2),
            (missing_chunk.message_id, 16),
            (missing_chunk.message_id, 17),
            (missing_chunk.message_id, 28),
        ];

        assert_eq!(missing_chunks, missing_map);
    }

    // Testing scenario where missing last chunk is identified
    #[test]
    pub fn test_find_missing_last_chunk() {
        let msg = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a CID".to_string(); 100],
        });
        let mut chunker = SimpleChunker::new(60);
        let mut chunks = chunker.chunk(msg).unwrap();

        let missing_chunk = chunks.pop().unwrap();
        let mut databuf = &missing_chunk[..missing_chunk.len()];
        let missing_chunk = if let Ok(chunk) = SimpleChunk::decode(&mut databuf) {
            chunk
        } else {
            panic!("decode removed chunk failed");
        };

        for chunk in chunks {
            let unchunk = chunker.unchunk(&chunk).unwrap();
            assert!(unchunk.is_none());
        }

        let missing_chunks = chunker.find_missing_chunks_map().unwrap();
        let mut missing_map = vec![(missing_chunk.message_id, missing_chunk.sequence_number)];

        assert_eq!(missing_chunks, missing_map);
    }

    // Testing getting missing chunk messages
    #[test]
    pub fn test_get_missing_chunk_messages() {
        let msg = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a CID".to_string(); 100],
        });
        let mut chunker = SimpleChunker::new(60);
        let mut chunks = chunker.chunk(msg).unwrap();

        let missing_chunk_raw = chunks.remove(2);

        let mut missing_chunks_raw = vec![missing_chunk_raw];

        // Remove a few more chunks for fun
        missing_chunks_raw.push(chunks.remove(15));
        missing_chunks_raw.push(chunks.remove(15));
        missing_chunks_raw.push(chunks.remove(25));

        for chunk in chunks {
            let unchunk = chunker.unchunk(&chunk).unwrap();
            assert!(unchunk.is_none());
        }

        let missing_chunks_messages = chunker
            .get_prev_sent_chunks(chunker.find_missing_chunks_map().unwrap())
            .unwrap();

        assert_eq!(missing_chunks_messages, missing_chunks_raw);
    }

    // Testing getting missing chunk messages size exceeding MTU
    #[test]
    pub fn test_get_missing_chunk_messages_respect_mtu() {
        let msg = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a CID".to_string(); 200],
        });
        let MTU = 60;
        let mut chunker = SimpleChunker::new(MTU);
        let mut chunks = chunker.chunk(msg).unwrap();

        let mut missing_chunks_raw: Vec<Vec<u8>> = vec![];

        // Remove a few more chunks for fun
        missing_chunks_raw.push(chunks.remove(15));
        missing_chunks_raw.push(chunks.remove(15));
        missing_chunks_raw.push(chunks.remove(25));
        missing_chunks_raw.push(chunks.remove(25));
        missing_chunks_raw.push(chunks.remove(25));
        missing_chunks_raw.push(chunks.remove(25));
        missing_chunks_raw.push(chunks.remove(25));
        missing_chunks_raw.push(chunks.remove(25));
        missing_chunks_raw.push(chunks.remove(25));
        missing_chunks_raw.push(chunks.remove(25));
        missing_chunks_raw.push(chunks.remove(25));
        missing_chunks_raw.push(chunks.remove(25));
        missing_chunks_raw.push(chunks.remove(25));

        for chunk in chunks {
            chunker.unchunk(&chunk).unwrap();
        }

        let missing_chunk_msgs = chunker.find_missing_chunks().unwrap();
        for msg in missing_chunk_msgs {
            assert!(msg.len() <= MTU.into());
        }
    }

    // Testing flow of send/receive missing chunks
    #[test]
    pub fn test_send_receive_with_missing_chunks() {
        let msg = Message::ApplicationAPI(ApplicationAPI::AvailableBlocks {
            cids: vec!["hello i am a CID".to_string(); 100],
        });
        let mut sending_chunker = SimpleChunker::new(60);
        let mut receiving_chunker = SimpleChunker::new(60);

        let mut chunks = sending_chunker.chunk(msg).unwrap();

        let mut missing_chunks_raw = vec![];
        missing_chunks_raw.push(chunks.remove(2));
        missing_chunks_raw.push(chunks.remove(15));
        missing_chunks_raw.push(chunks.remove(15));
        missing_chunks_raw.push(chunks.remove(25));

        for chunk in chunks {
            let unchunk = receiving_chunker.unchunk(&chunk).unwrap();
            assert!(unchunk.is_none());
        }

        let missing_chunks_map = receiving_chunker.find_missing_chunks().unwrap();
        let missing_chunks_map = missing_chunks_map.first().unwrap();
        let mut databuf = &missing_chunks_map[..missing_chunks_map.len()];
        let missing_chunks = if let Some(UnchunkResult::Missing(missing)) =
            sending_chunker.unchunk(&mut databuf).unwrap()
        {
            sending_chunker.get_prev_sent_chunks(missing.0).unwrap()
        } else {
            panic!("Failed to find missing message");
        };

        let mut found_msgs = 0;
        for chunk in missing_chunks {
            match receiving_chunker.unchunk(&chunk) {
                Ok(Some(UnchunkResult::Message(msg))) => {
                    found_msgs += 1;
                }
                Ok(other) => {}
                Err(_) => {}
            }
        }
        assert_eq!(found_msgs, 1);
    }
}

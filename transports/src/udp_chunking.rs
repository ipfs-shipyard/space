use crate::error::Result;
use log::{debug, error, info, trace, warn};
use messages::Message;
use parity_scale_codec::{Decode, Encode};
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use std::collections::BTreeMap;

use crate::chunking::MessageContainer;

#[derive(Clone, Debug, ParityDecode, ParityEncode)]
struct SimpleChunk {
    // Sender-specific 16-bit ID used to identify the message this chunk belongs to
    pub message_id: u16,
    // Sequence number indicates the order of reassembly
    #[codec(compact)]
    pub sequence_number: u64,
    // Data payload
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, ParityDecode, ParityEncode)]
enum Chunk {
    Leading(SimpleChunk),
    Final(SimpleChunk),
    Single(Vec<u8>),
}
impl Chunk {
    fn get_message_id(&self) -> u16 {
        self.as_simple_chunk().map(|c| c.message_id).unwrap_or(0)
    }
    fn get_sequence_number(&self) -> u16 {
        self.as_simple_chunk()
            .map(|c| c.sequence_number)
            .unwrap_or(0)
    }
    fn as_simple_chunk(&self) -> Option<&SimpleChunk> {
        match &self {
            Chunk::Single(_) => None,
            Chunk::Final(c) => Some(c),
            Chunk::Leading(c) => Some(c),
        }
    }
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
    recv_buffer: BTreeMap<u16, BTreeMap<u16, Chunk>>,
    // Last received message_id to optimize reassembly searching
    last_recv_msg_id: u16,
    pending: Vec<u16>,
    next_outgoing_msg_id: u16,
}

impl SimpleChunker {
    pub fn new(mtu: u16) -> Self {
        Self {
            mtu,
            recv_buffer: BTreeMap::<u16, BTreeMap<u16, Chunk>>::new(),
            last_recv_msg_id: 0,
            pending: Vec::new(),
            next_outgoing_msg_id: 0,
        }
    }

    fn recv_chunk(&mut self, chunk: Chunk) -> Result<()> {
        self.last_recv_msg_id = chunk.get_message_id();
        if let Some(msg_map) = self.recv_buffer.get_mut(&self.last_recv_msg_id) {
            msg_map.insert(chunk.get_sequence_number(), chunk);
        } else {
            let mut msg_map: BTreeMap<u16, Chunk> = BTreeMap::new();
            msg_map.insert(chunk.get_sequence_number(), chunk);
            self.recv_buffer.insert(self.last_recv_msg_id, msg_map);
        }

        Ok(())
    }

    fn attempt_msg_assembly(&mut self) -> Result<Option<Message>> {
        if let Some(result) = self.attempt_assemble(self.last_recv_msg_id)? {
            return Ok(Some(result));
        }
        if self.pending.is_empty() {
            warn!("This should not have occurred.");
            return Ok(None);
        }
        let index = rand::random::<u16>() as usize % self.pending.len();
        let id = self.pending[index];
        if let Some(result) = self.attempt_assemble(id)? {
            return Ok(Some(result));
        }
        if self.pending.len() > 0xFF {
            let id = self.pending.remove(0);
            error!("Giving up on receiving message {id}");
            self.drop_pending(id);
        }
        Ok(None)
    }
    fn attempt_assemble(&mut self, msg_id: u16) -> Result<Option<Message>> {
        let mut result = None;
        if let Some(msg_map) = self.recv_buffer.get(&self.last_recv_msg_id) {
            // The BTreeMap docs tell us that into_values will be an iter sorted by key
            // In this case the key is the sequence_number, so in a complete set of chunks
            // that means the last item in the iter (or now vec) should be the "final chunk"
            let last_chunk = msg_map.iter().last();
            // So to verify we have all message chunks...First grab the last chunk in the list
            match last_chunk {
                // Second, check if the last chunk has final_chunk set
                Some((last_seq, Chunk::Final(_))) => {
                    // Lastly, check if the final chunk's sequence number matches the number of chunks
                    if usize::from(*last_seq) == (msg_map.len() - 1) {
                        let chunks = msg_map.values().flat_map(Chunk::as_simple_chunk);
                        // If all those checks pass, then we *should* have all the chunks in order
                        // Now we attempt to assemble the message
                        result = Some(SimpleChunker::msg_unchunk(chunks)?);
                    }
                }
                Some((_, Chunk::Single(v))) => {
                    let data = v.clone();
                    result = Some(Message::decode(&mut data.as_slice())?);
                }
                _ => {}
            }
        }
        if result.is_some() {
            self.drop_pending(msg_id);
        }
        Ok(result)
    }
    fn drop_pending(&mut self, msg_id: u16) {
        self.recv_buffer.remove(&msg_id);
        self.pending.retain(|i| *i != msg_id);
    }
    fn msg_unchunk<'a, I: Iterator<Item = &'a SimpleChunk>>(data: I) -> Result<Message> {
        let mut all_data = vec![];
        data.for_each(|c| all_data.extend(&c.data));
        let mut databuf = &all_data[..all_data.len()];
        let container = MessageContainer::from_bytes(&mut databuf)?;
        Ok(container.message)
    }

    pub fn chunk(&mut self, message: Message) -> Result<Vec<Vec<u8>>> {
        let msg_id = self.next_outgoing_msg_id;
        if let Some(nxt_id) = self.next_outgoing_msg_id.checked_add(1u16) {
            self.next_outgoing_msg_id = nxt_id;
        } else {
            self.next_outgoing_msg_id = 0;
        }
        trace!("Message ID# {msg_id}={message:?}");
        let mut seq_num = 0;
        let message_bytes = if message.needs_envelope() {
            // Create container around message
            let container = MessageContainer::new(message);
            // Convert container into raw bytes
            container.to_bytes()
        } else {
            message.to_bytes()
        };
        if message_bytes.encoded_size() < self.mtu.into() {
            info!("Message {msg_id} fits in a single packet.");
            return Ok(vec![Chunk::Single(message_bytes).encode()]);
        }
        // Break bytes up into mtu-sized simple chunks
        let mut chunks = message_bytes
            .chunks(usize::from(self.mtu - CHUNK_OVERHEAD))
            .map(|data| {
                seq_num += 1;
                Chunk::Leading(SimpleChunk {
                    message_id: msg_id,
                    sequence_number: seq_num,
                    data: data.to_vec(),
                })
            })
            .collect::<Vec<Chunk>>();
        debug!("Message {msg_id} fits into {} packets.", chunks.len());
        // Set final to true in last chunk
        if let Some(last_chunk) = chunks.last_mut() {
            *last_chunk = Chunk::Final(last_chunk.as_simple_chunk().unwrap().clone());
        }
        // Encode all the chunks
        Ok(chunks.iter().map(|c| c.encode()).collect::<Vec<Vec<u8>>>())
    }

    pub fn unchunk(&mut self, data: &[u8]) -> Result<Option<Message>> {
        let mut databuf = &data[..data.len()];
        let chunk = Chunk::decode(&mut databuf)?;
        self.recv_chunk(chunk)?;
        self.attempt_msg_assembly()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use messages::ApplicationAPI;
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    #[test]
    pub fn test_chunk_overhead_against_const() {
        let data_sizes = [600, 2500, 5000, 10240];
        for size in data_sizes {
            let chunk = SimpleChunk {
                message_id: size,
                sequence_number: size,
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
            let mut chunker = SimpleChunker::new(mtu);
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

        let unchunked_message = chunker.unchunk(chunks.first().unwrap()).unwrap().unwrap();
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
            let unchunk = chunker.unchunk(&chunk).unwrap();
            assert!(unchunk.is_none());
        }
        let unchunked_msg = chunker.unchunk(&last_chunk).unwrap().unwrap();
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
            let unchunk = chunker.unchunk(&chunk).unwrap();
            assert!(unchunk.is_none());
        }
        let unchunked_msg = chunker.unchunk(&last_chunk).unwrap().unwrap();
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

        let unchunked_message = chunker
            .unchunk(msg_one_chunks.first().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(msg_one, unchunked_message);

        let unchunked_message = chunker
            .unchunk(msg_two_chunks.first().unwrap())
            .unwrap()
            .unwrap();
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
                Ok(Some(msg)) => {
                    assert!([&msg_one, &msg_two].contains(&&msg));
                    found_msgs += 1;
                }
                Ok(None) => {}
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
                Ok(Some(msg)) => {
                    assert!([&msg_one, &msg_two].contains(&&msg));
                    found_msgs += 1;
                }
                Ok(None) => {}
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
                Ok(Some(msg)) => {
                    assert!(msgs.contains(&msg));
                    found_msgs += 1;
                }
                Ok(None) => {}
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
}

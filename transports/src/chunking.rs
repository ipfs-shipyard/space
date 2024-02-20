use crate::error::{adhoc_err, Result};
use cid::multihash::{Code, MultihashDigest};
use log::error;
use messages::Message;
use parity_scale_codec::{Decode, Encode};
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use serde::Serialize;

const HASH_SIZE: usize = 16;

// This MessageContainer struct is intended to be used inside of the chunkers
// for verification of Message integrity during the chunking/assembly process
#[derive(Clone, Debug, ParityEncode, ParityDecode, Serialize, Eq, PartialEq)]
pub(crate) struct MessageContainer {
    // Hash of payload
    hash: [u8; HASH_SIZE],
    // Message payload
    pub message: Message,
}

impl MessageContainer {
    pub fn new(message: Message) -> Self {
        let hash = gen_hash(&message);
        // This hash uses a 128-bit Blake2s-128 hash, rather than the common sha2-256 to save on overhead size
        MessageContainer { hash, message }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.encode()
    }

    pub fn verify_cid(&self) -> Result<bool> {
        let regenerated_hash = gen_hash(&self.message);
        if regenerated_hash == self.hash {
            Ok(true)
        } else {
            error!(
                "Hash mismatch: provided={:?} deduced={:?}",
                self.hash, regenerated_hash
            );
            Ok(false)
        }
    }

    pub fn from_bytes(bytes: &mut &[u8]) -> Result<Self> {
        let container: MessageContainer = MessageContainer::decode(bytes)?;
        if !container.verify_cid()? {
            adhoc_err("Message container failed CID verification")?;
        }
        Ok(container)
    }
}

fn gen_hash(msg: &Message) -> [u8; HASH_SIZE] {
    let bytes = msg.to_bytes();
    Code::Blake2s128
        .digest(&bytes)
        .digest()
        .try_into()
        .expect("Hash is wrong size (should be constant since hash type is not changing)")
}

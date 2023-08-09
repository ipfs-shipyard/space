use crate::error::{adhoc_err, Result};
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use log::error;
use messages::Message;
use parity_scale_codec::{Decode, Encode};
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use serde::Serialize;

// This MessageContainer struct is intended to be used inside of the chunkers
// for verification of Message integrity during the chunking/assembly process
// Also all Messages are now transferred in IPLD blocks *tada*
#[derive(Clone, Debug, ParityEncode, ParityDecode, Serialize, Eq, PartialEq)]
pub(crate) struct MessageContainer {
    // CID using hash of payload (pre-serialized as Vec<u8>)
    cid: Vec<u8>,
    // Message payload
    pub message: Message,
}

impl MessageContainer {
    pub fn new(message: Message) -> Self {
        // This hash uses a 128-bit Blake2s-128 hash, rather than the common sha2-256 to save on overhead size
        let cid = MessageContainer::gen_cid(&message.to_bytes());
        MessageContainer {
            cid: cid.to_bytes(),
            message,
        }
    }

    // Generate a short-ish RAW CID from provided bytes
    pub fn gen_cid(bytes: &[u8]) -> Cid {
        let hash = Code::Blake2s128.digest(bytes);
        Cid::new_v1(0x55, hash)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.encode()
    }

    pub fn verify_cid(&self) -> Result<bool> {
        let original_cid = Cid::try_from(self.cid.clone())?;
        let regenerated_cid = MessageContainer::gen_cid(&self.message.to_bytes());
        if original_cid == regenerated_cid {
            Ok(true)
        } else {
            error!(
                "CID mismatch: provided={} deduced={}",
                original_cid.to_string(),
                regenerated_cid.to_string()
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

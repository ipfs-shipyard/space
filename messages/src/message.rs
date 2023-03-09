use crate::api::ApplicationAPI;
use crate::protocol::DataProtocol;
use crate::TransmissionBlock;

use anyhow::{bail, Result};
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
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
        let hash = Code::Blake2s128.digest(&message.to_bytes());
        let cid = Cid::new_v1(0x55, hash);
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
        Ok(original_cid == regenerated_cid)
    }

    pub fn from_bytes(bytes: &mut &[u8]) -> Result<Self> {
        let container: MessageContainer = MessageContainer::decode(bytes)?;
        if !container.verify_cid()? {
            bail!("Message container failed CID verification");
        }
        Ok(container)
    }
}

#[derive(Clone, Debug, ParityEncode, ParityDecode, Serialize, Eq, PartialEq)]
pub enum Message {
    DataProtocol(DataProtocol),
    ApplicationAPI(ApplicationAPI),
    Error(String),
}

impl Message {
    pub fn to_bytes(&self) -> Vec<u8> {
        self.encode()
    }

    pub fn available_blocks(cids: Vec<String>) -> Self {
        Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids })
    }
    pub fn request_available_blocks() -> Self {
        Message::ApplicationAPI(ApplicationAPI::RequestAvailableBlocks)
    }

    pub fn transmit_block(cid: &str, target_addr: &str) -> Self {
        Message::ApplicationAPI(ApplicationAPI::TransmitBlock {
            cid: cid.to_string(),
            target_addr: target_addr.to_string(),
        })
    }

    pub fn transmit_dag(cid: &str, target_addr: &str, retries: u8) -> Self {
        Message::ApplicationAPI(ApplicationAPI::TransmitDag {
            cid: cid.to_string(),
            target_addr: target_addr.to_string(),
            retries,
        })
    }

    pub fn import_file(path: &str) -> Self {
        Message::ApplicationAPI(ApplicationAPI::ImportFile {
            path: path.to_string(),
        })
    }

    pub fn export_dag(cid: &str, path: &str) -> Self {
        Message::ApplicationAPI(ApplicationAPI::ExportDag {
            cid: cid.to_string(),
            path: path.to_string(),
        })
    }

    pub fn get_missing_dag_blocks(cid: &str) -> Self {
        Message::ApplicationAPI(ApplicationAPI::GetMissingDagBlocks {
            cid: cid.to_string(),
        })
    }

    pub fn data_block(block: TransmissionBlock) -> Self {
        Message::DataProtocol(DataProtocol::Block(block))
    }

    pub fn request_missing_dag_blocks(cid: &str) -> Self {
        Message::DataProtocol(DataProtocol::RequestMissingDagBlocks {
            cid: cid.to_owned(),
        })
    }

    pub fn missing_dag_blocks(cid: &str, blocks: Vec<String>) -> Self {
        Message::DataProtocol(DataProtocol::MissingDagBlocks {
            cid: cid.to_owned(),
            blocks,
        })
    }
}

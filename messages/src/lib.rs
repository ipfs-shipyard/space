pub mod chunking;

use anyhow::{bail, Result};
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use clap::Subcommand;
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
        let regenerated_cid = MessageContainer::gen_cid(&self.message.clone().to_bytes());
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
    DataProtocol(TransmissionMessage),
    ApplicationAPI(ApplicationAPI),
}

impl Message {
    pub fn to_bytes(&self) -> Vec<u8> {
        self.encode()
    }
}

#[derive(Eq, PartialEq, Clone, Debug, ParityDecode, ParityEncode, Serialize)]
pub struct TransmissionBlock {
    pub cid: Vec<u8>,
    pub data: Vec<u8>,
    pub links: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, ParityDecode, ParityEncode, Serialize, Eq, PartialEq)]
pub enum TransmissionMessage {
    Block(TransmissionBlock),
}

#[derive(Clone, Debug, ParityEncode, ParityDecode, Serialize, Subcommand, Eq, PartialEq)]
pub enum ApplicationAPI {
    /// Asks IPFS instance to import a file path into the local IPFS store
    ImportFile { path: String },
    /// Response message to ImportFile containing file's root CID
    FileImported { path: String, cid: String },
    /// Asks IPFS instance to attempt to export a DAG to a file path
    ExportDag { cid: String, path: String },
    /// Tells IPFS instance whether comms are connected or not
    IsConnected { is_connected: bool },
    /// Asks IPFS instance if it has a DAG corresponding to the CID and all its child data
    IsDagComplete { cid: String },
    /// Chunks and initiates transmission of a file path to destination IP
    TransmitFile { path: String, target_addr: String },
    /// Initiates transmission of DAG corresponding to the given CID
    TransmitDag { cid: String, target_addr: String },
    /// Initiates transmission of block corresponding to the given CID
    TransmitBlock { cid: String, target_addr: String },
    /// Listens on address for data and writes out files received
    Receive { listen_addr: String },
    /// Verify that a block exists on the system and is valid
    ValidateBlack { cid: String },
    /// Information about the next pass used for calculating
    /// data transfer parameters
    NextPassInfo {
        duration: u32,
        send_bytes: u32,
        receive_bytes: u32,
    },
    /// Request for a DAG
    RequestDag { cid: String },
    /// Request for a block
    RequestBlock { cid: String },
    /// Request Available Blocks
    RequestAvailableBlocks,
    /// Advertise all available blocks by CID
    AvailableBlocks { cids: Vec<String> },
    /// Delete CID from local store
    DeleteCid { cid: String },
    /// Request available DAGs
    RequestAvailableDags,
    /// Advertise available DAGs as a map of CID to filename
    // AvailableDags { dags: BTreeMap<String, String> },
    /// Delete block from local store
    DeleteBlock { cid: String },
    /// Request missing DAG blocks
    GetMissingDagBlocks { cid: String },
    /// List of missing block CIDs
    MissingDagBlocks { blocks: Vec<String> },
}

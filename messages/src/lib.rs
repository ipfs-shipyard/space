use clap::Subcommand;
use parity_scale_codec::Encode;
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use serde::Serialize;

#[derive(Debug, ParityEncode, ParityDecode, Serialize)]
pub enum Message {
    DataProtocol(TransmissionMessage),
    ApplicationAPI(ApplicationAPI),
}

impl Message {
    pub fn to_bytes(self) -> Vec<u8> {
        self.encode()
    }
}

#[derive(Eq, PartialEq, Clone, Debug, ParityDecode, ParityEncode, Serialize)]
pub struct TransmissionChunk {
    pub cid_marker: Vec<u8>,
    pub chunk_offset: u16,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, ParityDecode, ParityEncode, Serialize)]
pub enum TransmissionMessage {
    Cid(Vec<u8>),
    Chunk(TransmissionChunk),
}

#[derive(Clone, Debug, ParityEncode, ParityDecode, Serialize, Subcommand)]
pub enum ApplicationAPI {
    /// Asks IPFS instance to import a file path into the local IPFS store
    ImportFile { path: String },
    /// Asks IPFS instance to attempt to export a CID to a file path
    ExportCid { cid: String },
    /// Tells IPFS instance whether comms are connected or not
    IsConnected { is_connected: bool },
    /// Asks IPFS instance if it has a given CID and all its child data
    IsCidComplete { cid: String },
    /// Chunks and transmits a file path to destination IP
    Transmit { path: String, target_addr: String },
    /// Listens on address for data and writes out files received
    Receive { listen_addr: String },
    /// Verify that CID exists on system and is valid with data
    ValidateCid { cid: String },
    /// Information about the next pass used for calculating
    /// data transfer parameters
    NextPassInfo {
        duration: u32,
        send_bytes: u32,
        receive_bytes: u32,
    },
    /// Request for a CID
    RequestCid { cid: String },
    /// Request Available CIDs
    RequestAvailableCids,
    /// Advertise new CIDs w/ description...eventually
    AvailableCids { cids: Vec<String> },
    /// Delete CID from local store
    DeleteCid { cid: String },
    /// Request remaining CID pieces or children
    RequestRemainingCidPieces { cid: String },
}

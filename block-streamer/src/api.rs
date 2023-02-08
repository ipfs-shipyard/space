use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::collection::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize, Subcommand)]
pub enum ApplicationAPI {
    /// Asks IPFS instance to import a file path into the local IPFS store
    ImportFile { path: String },
    /// Response message to ImportFile containing file's root CID
    ImportFileResult { path: String, cid: String },
    /// Asks IPFS instance to attempt to export a DAG to a file path
    ExportDag { cid: String, path: String },
    /// Tells IPFS instance whether comms are connected or not
    IsConnected { is_connected: bool },
    /// Asks IPFS instance if it has a DAG corresponding to the CID and all its child data
    IsDAGComplete { cid: String },
    /// Chunks and transmits a file path to destination IP
    Transmit { path: String, target_addr: String },
    // TODO: The Receive command will be deprecated by a general purpose "listening" mode
    /// Listens on address for data and write to file path
    Receive { path: String, listen_addr: String },
    /// Verify that a block exists on system and is valid
    ValidateBlock { cid: String },
    /// Information about the next pass used for calculating
    /// data transfer parameters
    NextPassInfo {
        duration: u32,
        send_bytes: u32,
        receive_bytes: u32,
    },
    /// Request for a block
    RequestBlock { cid: String },
    /// Request Available Blocks
    RequestAvailableBlocks,
    /// Advertise all available blocks by CID
    AvailableBlocks { cids: Vec<String> },
    /// Request available DAGs
    RequestAvailableDags,
    /// Advertise available DAGs as a map of CID to filename
    AvailableDags { dags: BTreeMap<String, String> },
    /// Delete block from local store
    DeleteBlock { cid: String },
    /// Request remaining CID pieces or children
    RequestRemainingDagBlocks { cid: String },
    /// List of remaining block CIDs
    RemainingDagBlocks { blocks: Vec<String> },
}

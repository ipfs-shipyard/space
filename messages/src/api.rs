use clap::Subcommand;
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use serde::Serialize;

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
    /// Asks IPFS instance if it has a valid DAG corresponding to the CID and all its child data
    ValidateDag { cid: String },
    /// Response to ValidateDag request, contains requested CID and a text response
    ValidateDagResponse { cid: String, result: String },
    /// Chunks and initiates transmission of a file path to destination IP
    TransmitFile { path: String, target_addr: String },
    /// Initiates transmission of DAG corresponding to the given CID
    TransmitDag { cid: String, target_addr: String },
    /// Initiates transmission of block corresponding to the given CID
    TransmitBlock { cid: String, target_addr: String },
    /// Listens on address for data and writes out files received
    Receive { listen_addr: String },
    /// Information about the next pass used for calculating
    /// data transfer parameters
    NextPassInfo {
        duration: u32,
        send_bytes: u32,
        receive_bytes: u32,
    },
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

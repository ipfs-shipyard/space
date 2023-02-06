use clap::Subcommand;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, Subcommand)]
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
    /// Listens on address for data and write to file path
    Receive { path: String, listen_addr: String },
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

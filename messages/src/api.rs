use clap::Subcommand;
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use serde::Serialize;

#[derive(Clone, Debug, ParityEncode, ParityDecode, Serialize, Eq, PartialEq)]
pub struct DagInfo {
    pub cid: String,
    pub filename: String,
}

#[derive(Clone, Debug, ParityEncode, ParityDecode, Serialize, Subcommand, Eq, PartialEq)]
pub enum ApplicationAPI {
    /// Asks IPFS instance to import a file path into the local IPFS store
    ImportFile {
        path: String,
    },
    /// Response message to ImportFile containing file's root CID
    #[command(skip)]
    FileImported {
        path: String,
        cid: String,
    },
    /// Asks IPFS instance to attempt to export a DAG to a file path
    ExportDag {
        cid: String,
        path: String,
    },
    /// Used to indicate the failure of a dag export
    #[command(skip)]
    DagExportFailed {
        cid: String,
        path: String,
        error: String,
    },
    /// Used to indicate a successful dag export
    #[command(skip)]
    DagExported {
        cid: String,
        path: String,
    },
    /// Sets current connected state
    SetConnected {
        #[arg(action(clap::ArgAction::Set), required(true))]
        connected: bool,
    },
    /// Requests the current connected state
    GetConnected,
    /// Response to GetConnected, with current connected state
    #[command(skip)]
    ConnectedState {
        connected: bool,
    },
    /// Asks IPFS instance if it has a valid DAG corresponding to the CID and all its child data
    ValidateDag {
        cid: String,
    },
    /// Response to ValidateDag request, contains requested CID and a text response
    #[command(skip)]
    ValidateDagResponse {
        cid: String,
        result: String,
    },
    // Initiates the transmission of a DAG corresponding to the given CID, with a given number of retries
    TransmitDag {
        cid: String,
        target_addr: String,
        retries: u8,
    },
    /// Indicates that a Dag has been transmitted completely successfully
    DagTransmissionComplete {
        cid: String,
    },
    /// Initiates transmission of block corresponding to the given CID
    TransmitBlock {
        cid: String,
        target_addr: String,
    },
    // Resumes the transmission of a dag which may have run out of retries or
    // paused due to connectivity lost
    ResumeTransmitDag {
        cid: String,
    },
    // Resumes the transmission of all dags which may be paused
    ResumeTransmitAllDags,
    /// Listens on address for data and writes out files received
    Receive {
        listen_addr: String,
    },
    /// Request Available Blocks
    RequestAvailableBlocks,
    /// Advertise all available blocks by CID
    #[command(skip)]
    AvailableBlocks {
        cids: Vec<String>,
    },
    /// Delete CID from local store
    DeleteCid {
        cid: String,
    },
    /// Delete block from local store
    DeleteBlock {
        cid: String,
    },
    /// Request missing DAG blocks
    GetMissingDagBlocks {
        cid: String,
    },
    /// List of missing blocks and associated DAG's CID
    #[command(skip)]
    MissingDagBlocks {
        cid: String,
        blocks: Vec<String>,
    },
    /// Requests current version of myceli
    RequestVersion {
        label: Option<String>,
    },
    /// Provides current version of myceli
    #[command(skip)]
    Version {
        version: String,
        rust: String,
        target: String,
        profile: String,
        features: Vec<String>,
        remote_label: Option<String>,
    },
    /// Request ALL available DAGs
    RequestAvailableDags,
    /// Request a list of named DAGs
    ListFiles,
    /// Lists available DAGs and corresponding filenames
    #[command(skip)]
    AvailableDags {
        dags: Vec<DagInfo>,
    },
    // TODO: Implement later
    // Information about the next pass used for calculating
    // data transfer parameters
    // NextPassInfo {
    //     duration: u32,
    //     send_bytes: u32,
    //     receive_bytes: u32,
    // },
    // Request available DAGs
    // RequestAvailableDags,
    // Advertise available DAGs as a map of CID to filename
    // AvailableDags { dags: BTreeMap<String, String> },
}

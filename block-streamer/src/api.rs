use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ApplicationAPI {
    /// Asks IPFS instance to import a file path into the local IPFS store
    ImportFile(String),
    /// Asks IPFS instance to attempt to export a CID to a file path
    ExportCid(String),
    /// Tells IPFS instance whether comms are connected or not
    IsConnected(bool),
    /// Asks IPFS instance if it has a given CID and all its child data
    IsCidComplete(String),
}

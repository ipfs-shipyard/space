use cid::Cid;
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use serde::Serialize;
use std::fmt;

#[derive(Eq, PartialEq, Clone, ParityDecode, ParityEncode, Serialize)]
pub struct TransmissionBlock {
    pub cid: Vec<u8>,
    pub data: Vec<u8>,
    pub links: Vec<Vec<u8>>,
}

impl fmt::Debug for TransmissionBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let cid_str = Cid::try_from(self.cid.clone())
            .map(|c| c.to_string())
            .unwrap();

        f.debug_struct("TransmissionBlock")
            .field("cid", &cid_str)
            .field("data", &self.data.len())
            .field("links", &self.links.len())
            .finish()
    }
}

#[derive(Clone, Debug, ParityDecode, ParityEncode, Serialize, Eq, PartialEq)]
pub enum DataProtocol {
    // Protocol level request for missing dag blocks
    RequestMissingDagBlocks {
        cid: String,
    },
    // Protocol level list of missing dag blocks
    MissingDagBlocks {
        cid: String,
        blocks: Vec<String>,
    },
    // Transmission message for individual block
    Block(TransmissionBlock),
    // Protocol level request for transmission of dag
    RequestTransmitDag {
        cid: String,
        target_addr: String,
        retries: u8,
    },
    // Protocol level request for transmission of block
    RequestTransmitBlock {
        cid: String,
        target_addr: String,
    },
    // This message is used inside of the protocol to initiate the re-requesting of missing dag blocks
    // in order to continue transmitting the dag
    RetryDagSession {
        cid: String,
        target_addr: String,
    },
    RetryDagWindowSession {
        cid: String,
        target_addr: String,
    },
    RequestTransmitDagWindow {
        cid: String,
        target_addr: String,
        retries: u8,
        window_size: u8,
    },
    // Message to request list of blocks missing from list of CIDs sent
    RequestMissingDagBlocksWindow {
        cid: String,
        blocks: Vec<String>,
    },
    MissingDagBlocksWindow {
        cid: String,
        blocks: Vec<String>,
    },
}

use crate::api::ApplicationAPI;
use crate::protocol::DataProtocol;
use crate::TransmissionBlock;

use parity_scale_codec::Encode;
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use serde::Serialize;

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

    pub fn to_hex(&self) -> String {
        let mut hex_str = String::new();

        for b in self.to_bytes() {
            hex_str = format!("{}{:02X}", hex_str, b);
        }

        hex_str
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

    pub fn request_missing_dag_window_blocks(cid: &str, blocks: Vec<String>) -> Self {
        Message::DataProtocol(DataProtocol::RequestMissingDagWindowBlocks {
            cid: cid.to_owned(),
            blocks,
        })
    }

    pub fn missing_dag_blocks(cid: &str, blocks: Vec<String>) -> Self {
        Message::DataProtocol(DataProtocol::MissingDagBlocks {
            cid: cid.to_owned(),
            blocks,
        })
    }

    pub fn request_version() -> Self {
        Message::ApplicationAPI(ApplicationAPI::RequestVersion)
    }
}

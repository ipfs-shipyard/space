use crate::{
    api::ApplicationAPI,
    cid_list,
    err::{Error, Result},
    sync::{PushMessage, SyncMessage},
};
#[cfg(feature = "proto_ship")]
use crate::{protocol::DataProtocol, TransmissionBlock};
use parity_scale_codec::Encode;
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use serde::Serialize;

#[derive(Clone, Debug, ParityEncode, ParityDecode, Serialize, Eq, PartialEq)]
pub struct Unsupported {}
#[derive(Clone, Debug, ParityEncode, ParityDecode, Serialize, Eq, PartialEq)]
pub enum Message {
    #[cfg(feature = "proto_ship")]
    DataProtocol(DataProtocol),
    #[cfg(not(feature = "proto_ship"))]
    DataProtocol(Unsupported),

    ApplicationAPI(ApplicationAPI),
    Error(String),

    Sync(SyncMessage),
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

    // All functions below are helper functions for generating messages

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

    #[cfg(feature = "proto_ship")]
    pub fn data_block(block: TransmissionBlock) -> Self {
        Message::DataProtocol(DataProtocol::Block(block))
    }

    #[cfg(feature = "proto_ship")]
    pub fn request_missing_dag_blocks(cid: &str) -> Self {
        Message::DataProtocol(DataProtocol::RequestMissingDagBlocks {
            cid: cid.to_owned(),
        })
    }

    #[cfg(feature = "proto_ship")]
    pub fn request_missing_dag_window_blocks(cid: &str, blocks: Vec<String>) -> Self {
        Message::DataProtocol(DataProtocol::RequestMissingDagWindowBlocks {
            cid: cid.to_owned(),
            blocks,
        })
    }

    #[cfg(feature = "proto_ship")]
    pub fn missing_dag_blocks(cid: &str, blocks: Vec<String>) -> Self {
        Message::DataProtocol(DataProtocol::MissingDagBlocks {
            cid: cid.to_owned(),
            blocks,
        })
    }

    pub fn request_version(resp_label: String) -> Self {
        Message::ApplicationAPI(ApplicationAPI::RequestVersion {
            label: Some(resp_label),
        })
    }

    pub fn push(cids: cid_list::CompactList, name: String) -> Result<Self> {
        if cids.is_empty() {
            Err(Error::EmptyCidList)
        } else {
            Ok(Self::Sync(SyncMessage::Push(PushMessage::new(cids, name))))
        }
    }

    #[cfg(feature = "proto_sync")]
    pub fn pull(cids: cid_list::CompactList) -> Self {
        Self::Sync(SyncMessage::Pull(cids))
    }

    pub fn block(block_bytes: Vec<u8>) -> Self {
        Self::Sync(SyncMessage::Block(block_bytes))
    }

    pub fn needs_envelope(&self) -> bool {
        !matches!(self, Self::Sync(_))
    }

    pub fn fit_size(within: u16) -> u16 {
        let mut v = vec![0u8; within as usize - crate::PUSH_OVERHEAD];
        loop {
            if Self::block(v.clone()).encoded_size() < within.into() {
                if let Ok(result) = v.len().try_into() {
                    return result;
                } else {
                    v.pop();
                }
            } else {
                v.pop();
            }
        }
    }
    pub fn name(&self) -> &'static str {
        match &self {
            Self::DataProtocol(_) => "Data",
            Self::ApplicationAPI(_) => "API",
            Self::Error(_) => "Error",
            Self::Sync(_m) => {
                #[cfg(feature = "proto_sync")]
                {
                    _m.name()
                }
                #[cfg(not(feature = "proto_sync"))]
                "UnsupportedSyncMessage"
            }
        }
    }

    pub fn target_addr(&self) -> Option<String> {
        match &self {
            Self::ApplicationAPI(ApplicationAPI::TransmitBlock { target_addr, .. }) => {
                Some(target_addr.clone())
            }
            Self::ApplicationAPI(ApplicationAPI::TransmitDag { target_addr, .. }) => {
                Some(target_addr.clone())
            }
            _ => None,
        }
    }
}

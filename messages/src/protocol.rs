use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use serde::Serialize;

#[derive(Eq, PartialEq, Clone, Debug, ParityDecode, ParityEncode, Serialize)]
pub struct TransmissionBlock {
    pub cid: Vec<u8>,
    pub data: Vec<u8>,
    pub links: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, ParityDecode, ParityEncode, Serialize, Eq, PartialEq)]
pub enum DataProtocol {
    Block(TransmissionBlock),
}

use crate::cid_list;
use cid::multihash;
use cid::multihash::Hasher;
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use serde::Serialize;
use std::fmt::{Debug, Formatter};

const HASH_SIZE: usize = 16;
pub const PUSH_OVERHEAD: usize = HASH_SIZE + 1;

pub type HashCheck = [u8; HASH_SIZE];

#[derive(Clone, ParityEncode, ParityDecode, Serialize, Eq, PartialEq)]
pub enum SyncMessage {
    Push(PushMessage),           //I have these CIDs, you may pull them.
    Pull(cid_list::CompactList), //I do not have these CIDs, maybe you could send their blocks to me
    Ack(cid_list::CompactList),  //I *also* have these CIDs, stop pushing them
    Block(Vec<u8>),              //Here's the data for a block.
}

impl SyncMessage {
    pub fn name(&self) -> &'static str {
        match &self {
            Self::Push(_) => "Push",
            Self::Pull(_) => "Pull",
            Self::Ack(_) => "Ack",
            Self::Block(_) => "Block",
        }
    }
}

impl Debug for SyncMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Push(x) => write!(f, "Push({x:?})"),
            Self::Pull(x) => write!(f, "Pull({x:?})"),
            Self::Ack(x) => write!(f, "Ack({x:?})"),
            Self::Block(x) => write!(f, "Block({}B)", x.len()),
        }
    }
}

#[derive(Clone, ParityEncode, ParityDecode, Serialize, Eq, PartialEq)]
pub struct PushMessage {
    pub first_cid_name: String,
    pub cids: cid_list::CompactList,
    //A corrupted pull has a modest negative impact, but a corrupted push can begin a search for a
    //  CID that points to something which may never have actually existed in the first place.
    //  Adding this hashing of the CIDs to detect corruption.
    hash: HashCheck,
}
impl PushMessage {
    pub fn new(cids: cid_list::CompactList, first_cid_name: String) -> Self {
        let hash = Self::do_hash(&cids);
        Self {
            first_cid_name,
            cids,
            hash,
        }
    }
    pub fn check(&self) -> bool {
        self.hash == Self::do_hash(&self.cids)
    }
    fn do_hash(cids: &cid_list::CompactList) -> HashCheck {
        let mut hasher = multihash::Blake2s128::default();
        for d in cids {
            hasher.update(&d.to_bytes());
        }
        let digest_slice = hasher.finalize();
        digest_slice.try_into().unwrap()
    }
}
impl Debug for PushMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "PushMsg(")?;
        if !self.first_cid_name.is_empty() {
            write!(f, "{}=", &self.first_cid_name)?;
        }
        write!(f, "{:?})", &self.cids)
    }
}

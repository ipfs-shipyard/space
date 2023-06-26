pub mod balanced_tree;
pub mod builder;
pub mod chunker;
pub mod codecs;
mod types;
pub mod unixfs;

pub use crate::types::{Block, Link, LinkRef, Links, LoadedCid, PbLinks, Source};

use crate::codecs::Codec;
use anyhow::{bail, Context as _, Result};
use cid::Cid;
use libipld::{prelude::Codec as _, Ipld, IpldCodec};

/// Extract links from the given content.
///
/// Links will be returned as a sorted vec
pub fn parse_links(cid: &Cid, bytes: &[u8]) -> Result<Vec<Cid>> {
    let codec = Codec::try_from(cid.codec()).context("unknown codec")?;
    let mut cids = vec![];
    let codec = match codec {
        Codec::DagCbor => IpldCodec::DagCbor,
        Codec::DagPb => IpldCodec::DagPb,
        Codec::DagJson => IpldCodec::DagJson,
        Codec::Raw => IpldCodec::Raw,
        _ => bail!("unsupported codec {:?}", codec),
    };
    codec.references::<Ipld, _>(bytes, &mut cids)?;
    cids.sort();
    Ok(cids)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    pub fn file_with_repeat_chunks() {
        let bytes : &[u8] = &[0x12, 0x2a, 0x0a, 0x24, 0x01, 0x55, 0x12, 0x20, 0x8f, 0x43, 0x43, 0x46,
            0x64, 0x8f, 0x6b, 0x96, 0xdf, 0x89, 0xdd, 0xa9, 0x01, 0xc5, 0x17, 0x6b,
            0x10, 0xa6, 0xd8, 0x39, 0x61, 0xdd, 0x3c, 0x1a, 0xc8, 0x8b, 0x59, 0xb2,
            0xdc, 0x32, 0x7a, 0xa4, 0x12, 0x00, 0x18, 0x02, 0x12, 0x2a, 0x0a, 0x24,
            0x01, 0x55, 0x12, 0x20, 0x8f, 0x43, 0x43, 0x46, 0x64, 0x8f, 0x6b, 0x96,
            0xdf, 0x89, 0xdd, 0xa9, 0x01, 0xc5, 0x17, 0x6b, 0x10, 0xa6, 0xd8, 0x39,
            0x61, 0xdd, 0x3c, 0x1a, 0xc8, 0x8b, 0x59, 0xb2, 0xdc, 0x32, 0x7a, 0xa4,
            0x12, 0x00, 0x18, 0x02, 0x0a, 0x08, 0x08, 0x02, 0x18, 0x04, 0x20, 0x02,
            0x20, 0x02];
        let cid : Cid = "bafybeiegfwauaenc4pa7jqfssar4i4pafsul4g62e3av64fwir5uodv7q4".try_into().unwrap();
        let actual = parse_links(&cid, bytes).unwrap();
        let child : Cid = "bafkreiepinbumzepnoln7co5vea4kf3lcctnqolb3u6bvsellgznymt2uq".try_into().unwrap();
        let expected = [child.clone(), child.clone()];
        assert_eq!(actual, expected);
    }
}

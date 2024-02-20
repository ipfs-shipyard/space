use crate::err::{Error, Result};
use cid::{multihash::Multihash, Cid};
use ipfs_unixfs::codecs::Codec;
use parity_scale_codec::{Compact, CompactLen, Encode};
use parity_scale_codec_derive::{Decode as ParityDecode, Encode as ParityEncode};
use serde::Serialize;
use std::fmt::{Debug, Formatter};

#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    ParityEncode,
    ParityDecode,
    Serialize,
    Default,
    Ord,
    PartialOrd,
)]
pub struct Meta {
    #[codec(compact)]
    codec: u64,
    #[codec(compact)]
    algo: u64,
    // digest_len: u8,
}

#[derive(Clone, ParityEncode, ParityDecode, Serialize, Eq, PartialEq, Default)]
pub struct CompactList {
    meta: Meta,
    digests: Vec<Vec<u8>>,

    #[codec(skip)]
    size: usize,
}

impl CompactList {
    fn assign(&mut self, cid: &Cid) -> Result<()> {
        let (meta, hash) = Meta::new(cid)?;
        self.digests = vec![hash.digest().into()];
        self.meta = meta;
        self.size = self.encoded_size();
        Ok(())
    }
    pub fn contains(&self, cid: &Cid) -> bool {
        if let Ok((m, h)) = Meta::new(cid) {
            if m != self.meta {
                return false;
            }
            self.contains_digest(h.digest())
        } else {
            false
        }
    }
    pub fn include(&mut self, cid: &Cid, sz: usize) -> bool {
        if self.size == 0 {
            self.assign(cid).is_ok()
        } else if let Ok((m, h)) = Meta::new(cid) {
            if m != self.meta {
                return false;
            }
            let digest = h.digest();
            if self.contains_digest(digest) {
                return true;
            }
            let delta = digest.len() + len_len(digest.len()) + len_len(self.digests.len() + 1)
                - len_len(self.digests.len());
            if self.size + delta <= sz {
                self.digests.push(digest.into());
                self.size += delta;
                true
            } else {
                false
            }
        } else {
            false
        }
    }
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
    pub fn shared_traits(&self) -> Meta {
        self.meta
    }
    fn contains_digest(&self, digest: &[u8]) -> bool {
        self.digests.iter().any(|d| d.as_slice() == digest)
    }
    pub fn built_size(&self) -> usize {
        self.size
    }
}
impl TryFrom<&Cid> for CompactList {
    type Error = Error;

    fn try_from(value: &Cid) -> Result<Self> {
        let mut result = CompactList::default();
        result.assign(value)?;
        Ok(result)
    }
}
impl TryFrom<Cid> for CompactList {
    type Error = Error;

    fn try_from(value: Cid) -> Result<Self> {
        Self::try_from(&value)
    }
}
impl Debug for CompactList {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Ok(cdc) = Codec::try_from(self.meta.codec) {
            write!(f, "CIDs({:?} ", cdc)?;
        }
        if let Ok(alg) = cid::multihash::Code::try_from(self.meta.algo) {
            if alg != cid::multihash::Code::Sha2_256 {
                write!(f, "{alg:?} ")?;
            }
        }
        if let Some(c) = self.into_iter().next() {
            let l = self.digests.len();
            write!(f, "{};[N={l}])", c)
        } else {
            write!(f, "EMPTY!)")
        }
    }
}

fn len_len(i: usize) -> usize {
    let i: u64 = i.try_into().unwrap_or(u64::MAX);
    Compact::<u64>::compact_len(&i)
}

impl Meta {
    fn new(cid: &Cid) -> Result<(Self, Multihash)> {
        if cid.version() == cid::Version::V0 {
            return Self::new(&cid.into_v1()?);
        }
        let h = cid.hash();
        let me = Self {
            codec: cid.codec(),
            algo: h.code(),
            // digest_len: h.size(),
        };
        Ok((me, *h))
    }
}

impl TryFrom<&Cid> for Meta {
    type Error = Error;

    fn try_from(value: &Cid) -> Result<Self> {
        Self::new(value).map(|x| x.0)
    }
}
impl TryFrom<Cid> for Meta {
    type Error = Error;

    fn try_from(value: Cid) -> Result<Self> {
        Self::try_from(&value)
    }
}

impl<'a> IntoIterator for &'a CompactList {
    type Item = Cid;
    type IntoIter = CompactListIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        CompactListIter { l: self, i: 0 }
    }
}

#[derive(Clone, Copy)]
pub struct CompactListIter<'a> {
    l: &'a CompactList,
    i: usize,
}

impl<'a> Iterator for CompactListIter<'a> {
    type Item = Cid;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i < self.l.digests.len() {
            let h = Multihash::wrap(self.l.meta.algo, &self.l.digests[self.i]).ok()?;
            let result = Cid::new(cid::Version::V1, self.l.meta.codec, h).ok()?;
            self.i += 1;
            Some(result)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_sizes() {
        let mut t: CompactList = Cid::try_from("QmfYEZk4qQNFUemHDwRZe9Cxg1U8aMhhAsLFz3JXBvn4WL")
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(t.meta.codec, 0x70);
        const CHUNK_SIZE: usize = 500;
        //These are all V0 and thus all the meta is shared
        let cids = &[
            "QmYMq3DPTgD1pNvprFryigLeHvbDzGMZrmLovgAGNgdtVj",
            "QmSBjoLPJtDm7XfDrHuAkLe3fFjUmeDJ9EusAG4Q5zozsa",
            "QmSjBJV94TzRpvPcDsT4zM23VLvHuSKGVRbSvBf7HSHR2h",
            "QmPEuX1jLFFEw8Cps9hrRhENt2bo6z6sCp2vdzRRCZui2w",
            "QmThhbkE7WunooPSU2YxmYqJTbt2NJMz43noBwyhVPLUGU",
            "QmQyfg1KWwNyrgTq2MEHnz5bMgWVaUhQ79AHV8DxMz3Egy",
            "QmcbR8nUYKhy7bc93K5PoP1hHLeLipXUJJZ6cNurUrms6Q",
            "QmSS5Ecov1VxxRAA5fYBsQYSPzq15GL7yntw99R4D8ehSH",
            "QmVVrrUkqNECz3qF6HeBgzMfQo75zswpp5Ux6fEAQEDHqi",
            "QmRrAYCw9Gwi1hDsEsJf9gutW7Xy5aFLezpsqaZFSqyshA",
            "QmebrayY6dntCg7mDp7GSycLPjCb7PStqipt5zrgG9y9cA",
            "QmeNiriJ7ou4Cn1tb6P5ratfhTQRMFZxbqbkM5dbcDejoZ",
            "QmQC5dCzH5smMAcVdPuyMKq2zJgHHQzf4d2Dq3Bcyc1s5Y",
            "QmWPaG8xhbonT2jnzBF78X7emd82imMfStMaM22pttMw8j",
        ];
        for c in cids {
            assert_eq!(t.size, t.encoded_size());
            let c = Cid::try_from(*c).unwrap();
            assert!(t.include(&c, CHUNK_SIZE), "{c:?}");
        }
        assert!(t.encoded_size() <= CHUNK_SIZE);
    }
}

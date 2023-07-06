use std::{
    collections::VecDeque,
    fmt::Debug,
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::{anyhow, bail, ensure, Result};
use bytes::{Buf, Bytes};
use cid::{multihash::MultihashDigest, Cid};
use futures::{future::BoxFuture, stream::BoxStream, Stream};
use prost::Message;

use crate::{
    chunker::DEFAULT_CHUNK_SIZE_LIMIT,
    codecs::Codec,
    types::{Block, Link, LinkRef, Links, PbLinks},
};

pub mod unixfs_pb {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/unixfs_pb.rs"));
}

pub mod dag_pb {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/merkledag_pb.rs"));
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive,
)]
#[repr(i32)]
pub enum DataType {
    Raw = 0,
    Directory = 1,
    File = 2,
    Metadata = 3,
    Symlink = 4,
}

#[derive(Debug, Clone)]
pub struct Unixfs {
    inner: unixfs_pb::Data,
}

impl Unixfs {
    pub fn from_bytes<B: Buf>(bytes: B) -> Result<Self> {
        let proto = unixfs_pb::Data::decode(bytes)?;

        Ok(Unixfs { inner: proto })
    }

    pub fn typ(&self) -> DataType {
        self.inner.r#type.try_into().expect("invalid data type")
    }

    pub fn data(&self) -> Option<&Bytes> {
        self.inner.data.as_ref()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum UnixfsNode {
    Raw(Bytes),
    RawNode(Node),
    Directory(Node),
    File(Node),
    Symlink(Node),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Node {
    pub outer: dag_pb::PbNode,
    pub inner: unixfs_pb::Data,
}

impl Node {
    fn encode(&self) -> Result<Bytes> {
        let bytes = self.outer.encode_to_vec();
        Ok(bytes.into())
    }

    pub fn typ(&self) -> DataType {
        self.inner.r#type.try_into().expect("invalid data type")
    }

    pub fn data(&self) -> Option<Bytes> {
        self.inner.data.clone()
    }

    pub fn filesize(&self) -> Option<u64> {
        self.inner.filesize
    }

    pub fn blocksizes(&self) -> &[u64] {
        &self.inner.blocksizes
    }

    pub fn size(&self) -> Option<usize> {
        if self.outer.links.is_empty() {
            return Some(
                self.inner
                    .data
                    .as_ref()
                    .map(|d| d.len())
                    .unwrap_or_default(),
            );
        }

        None
    }

    pub fn links(&self) -> Links {
        match self.typ() {
            DataType::Raw => Links::RawNode(PbLinks::new(&self.outer)),
            DataType::Directory => Links::Directory(PbLinks::new(&self.outer)),
            DataType::File => Links::File(PbLinks::new(&self.outer)),
            DataType::Symlink => Links::Symlink(PbLinks::new(&self.outer)),
            DataType::Metadata => unimplemented!(),
        }
    }
}

impl UnixfsNode {
    pub fn decode(cid: &Cid, buf: Bytes) -> Result<Self> {
        Self::decode_from_codec(cid.codec().try_into()?, buf)
    }
    pub fn decode_from_codec(codec: Codec, buf: Bytes) -> Result<Self> {
        match codec {
            c if c == Codec::Raw => Ok(UnixfsNode::Raw(buf)),
            _ => {
                let outer = dag_pb::PbNode::decode(buf)?;
                let inner_data = outer
                    .data
                    .as_ref()
                    .cloned()
                    .ok_or_else(|| anyhow!("missing data"))?;
                let inner = unixfs_pb::Data::decode(inner_data)?;
                let typ: DataType = inner.r#type.try_into()?;
                let node = Node { outer, inner };

                // ensure correct unixfs type
                match typ {
                    DataType::Raw => todo!(),
                    DataType::Directory => Ok(UnixfsNode::Directory(node)),
                    DataType::File => Ok(UnixfsNode::File(node)),
                    DataType::Symlink => Ok(UnixfsNode::Symlink(node)),
                    DataType::Metadata => bail!("unixfs metadata is not supported"),
                }
            }
        }
    }

    pub fn encode(&self) -> Result<Block> {
        let res = match self {
            UnixfsNode::Raw(data) => {
                let out = data.clone();
                let links = vec![];
                let cid = Cid::new_v1(Codec::Raw as _, cid::multihash::Code::Sha2_256.digest(&out));
                Block::new(cid, out, links)
            }
            UnixfsNode::RawNode(node)
            | UnixfsNode::Directory(node)
            | UnixfsNode::File(node)
            | UnixfsNode::Symlink(node) => {
                let out = node.encode()?;
                let links = node
                    .links()
                    .map(|x| Ok(x?.cid))
                    .collect::<Result<Vec<_>>>()?;
                let cid = Cid::new_v1(
                    Codec::DagPb as _,
                    cid::multihash::Code::Sha2_256.digest(&out),
                );
                Block::new(cid, out, links)
            }
        };

        ensure!(
            res.data().len() <= DEFAULT_CHUNK_SIZE_LIMIT,
            "node is too large: {} bytes",
            res.data().len()
        );

        Ok(res)
    }

    pub const fn typ(&self) -> Option<DataType> {
        match self {
            UnixfsNode::Raw(_) => None,
            UnixfsNode::RawNode(_) => Some(DataType::Raw),
            UnixfsNode::Directory(_) => Some(DataType::Directory),
            UnixfsNode::File(_) => Some(DataType::File),
            UnixfsNode::Symlink(_) => Some(DataType::Symlink),
        }
    }

    /// Returns the size in bytes of the underlying data.
    /// Available only for `Raw` and `File` which are a single block with no links.
    pub fn size(&self) -> Option<usize> {
        match self {
            UnixfsNode::Raw(data) => Some(data.len()),
            UnixfsNode::Directory(node)
            | UnixfsNode::RawNode(node)
            | UnixfsNode::File(node)
            | UnixfsNode::Symlink(node) => node.size(),
        }
    }

    /// Returns the filesize in bytes.
    /// Should only be set for `Raw` and `File`.
    pub fn filesize(&self) -> Option<u64> {
        match self {
            UnixfsNode::Raw(data) => Some(data.len() as u64),
            UnixfsNode::Directory(node)
            | UnixfsNode::RawNode(node)
            | UnixfsNode::File(node)
            | UnixfsNode::Symlink(node) => node.filesize(),
        }
    }

    /// Returns the blocksizes of the links
    /// Should only be set for File
    pub fn blocksizes(&self) -> &[u64] {
        match self {
            UnixfsNode::Raw(_) => &[],
            UnixfsNode::Directory(node)
            | UnixfsNode::RawNode(node)
            | UnixfsNode::Symlink(node)
            | UnixfsNode::File(node) => node.blocksizes(),
        }
    }

    pub fn links(&self) -> Links<'_> {
        match self {
            UnixfsNode::Raw(_) => Links::Raw,
            UnixfsNode::RawNode(node) => Links::RawNode(PbLinks::new(&node.outer)),
            UnixfsNode::Directory(node) => Links::Directory(PbLinks::new(&node.outer)),
            UnixfsNode::File(node) => Links::File(PbLinks::new(&node.outer)),
            UnixfsNode::Symlink(node) => Links::Symlink(PbLinks::new(&node.outer)),
        }
    }

    pub fn links_owned(&self) -> Result<VecDeque<Link>> {
        self.links().map(|l| l.map(|l| l.to_owned())).collect()
    }

    pub const fn is_dir(&self) -> bool {
        matches!(self, Self::Directory(_))
    }

    pub async fn get_link_by_name<S: AsRef<str>>(
        &self,
        link_name: S,
    ) -> Result<Option<LinkRef<'_>>> {
        let link_name = link_name.as_ref();
        self.links()
            .find(|l| match l {
                Ok(l) => l.name == Some(link_name),
                _ => false,
            })
            .transpose()
    }

    pub fn symlink(&self) -> Result<Option<&str>> {
        if let Self::Symlink(ref node) = self {
            let link = std::str::from_utf8(node.inner.data.as_deref().unwrap_or_default())?;
            Ok(Some(link))
        } else {
            Ok(None)
        }
    }
}

pub enum UnixfsChildStream<'a> {
    Directory { stream: BoxStream<'a, Result<Link>> },
}

impl<'a> Debug for UnixfsChildStream<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnixfsChildStream::Directory { .. } => write!(
                f,
                "UnixfsChildStream::Directory {{ stream: BoxStream<Result<Link>>}}"
            ),
        }
    }
}

impl Stream for UnixfsChildStream<'_> {
    type Item = Result<Link>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut *self {
            UnixfsChildStream::Directory { stream, .. } => Pin::new(stream).poll_next(cx),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            UnixfsChildStream::Directory { stream, .. } => stream.size_hint(),
        }
    }
}

pub fn read_data_to_buf(
    pos: &mut usize,
    pos_max: Option<usize>,
    data: &[u8],
    buf: &mut tokio::io::ReadBuf<'_>,
) -> usize {
    let data_to_read = pos_max.map(|pos_max| pos_max - *pos).unwrap_or(data.len());
    let amt = std::cmp::min(std::cmp::min(data_to_read, buf.remaining()), data.len());
    buf.put_slice(&data[..amt]);
    *pos += amt;
    amt
}

pub fn find_block(node: &UnixfsNode, pos: u64, node_offset: u64) -> (u64, Option<usize>) {
    let pivots = node
        .blocksizes()
        .iter()
        .scan(node_offset, |state, &x| {
            *state += x;
            Some(*state)
        })
        .collect::<Vec<_>>();
    let block_index = match pivots.binary_search(&pos) {
        Ok(b) => b + 1,
        Err(b) => b,
    };
    if block_index < pivots.len() {
        let next_node_offset = if block_index > 0 {
            pivots[block_index - 1]
        } else {
            node_offset
        };
        (next_node_offset, Some(block_index))
    } else {
        (pivots[pivots.len() - 1], None)
    }
}

#[allow(clippy::large_enum_variant)]
pub enum CurrentNodeState {
    // Initial state
    Outer,
    // Need to load next node from the list
    NextNodeRequested {
        next_node_offset: usize,
    },
    // Node has been loaded and ready to be processed
    Loaded {
        node_offset: usize,
        node_pos: usize,
        node: UnixfsNode,
    },
    // Ongoing loading of the node
    Loading {
        node_offset: usize,
        fut: BoxFuture<'static, Result<UnixfsNode>>,
    },
}

impl Debug for CurrentNodeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CurrentNodeState::Outer => write!(f, "CurrentNodeState::Outer"),
            CurrentNodeState::NextNodeRequested { next_node_offset } => {
                write!(f, "CurrentNodeState::None ({next_node_offset})")
            }
            CurrentNodeState::Loaded {
                node_offset,
                node_pos,
                node,
            } => {
                write!(
                    f,
                    "CurrentNodeState::Loaded({node_offset:?}, {node_pos:?}, {node:?})"
                )
            }
            CurrentNodeState::Loading { .. } => write!(f, "CurrentNodeState::Loading(Fut)"),
        }
    }
}

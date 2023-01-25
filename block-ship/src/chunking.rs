use crate::types::BlockWrapper;

use anyhow::Result;
use cid::Cid;
use futures::TryStreamExt;
use iroh_unixfs::{
    builder::{File, FileBuilder},
    Block,
};
use parity_scale_codec::Encode;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio::fs::File as TokioFile;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

// TODO: Refactor this and chunks_to_path so that they actually do what their names say
// and so that they can be tested directly against each other
pub async fn path_to_chunks(path: &PathBuf) -> Result<Vec<Vec<u8>>> {
    let file: File = FileBuilder::new()
        .path(path)
        .fixed_chunker(50)
        // This will decrease the width of the underlying tree
        // but the logic isn't ready on the receiving end end
        // and the current CID size means that two links will
        // still overrun the lab radio packet size
        // .degree(2)
        .build()
        .await?;

    let mut blocks: Vec<_> = file.encode().await?.try_collect().await?;

    let mut payloads = vec![];

    info!("{:?} broken into {} blocks", path.as_path(), blocks.len());

    // This randomly shuffles the order of the blocks (prior to chunking)
    // in order to exercise reassembly on the receiver side.
    blocks.shuffle(&mut thread_rng());

    for block in blocks {
        let wrapper = BlockWrapper::from_block(block)?;
        let chunks = wrapper.to_chunks()?;
        for c in chunks {
            payloads.push(c.encode());
        }
    }

    Ok(payloads)
}

pub async fn chunks_to_path(
    path: &PathBuf,
    root: &Block,
    blocks: &BTreeMap<Cid, Block>,
) -> Result<bool> {
    // First check if all CIDs exist
    for c in root.links().iter() {
        if !blocks.contains_key(c) {
            info!("Missing cid {}, wait for more data", c);
            return Ok(false);
        }
    }

    let mut output_file = TokioFile::create(path).await?;
    for cid in root.links().iter() {
        if let Some(data) = blocks.get(cid) {
            output_file.write_all(data.data()).await?;
        } else {
            // missing a cid...not ready yet...we shouldn't get
            // here because of the CIDs check above, but
            // we verify again anyways
            warn!("Still missing a cid...");
            return Ok(false);
        }
    }
    output_file.flush().await?;

    Ok(true)
}

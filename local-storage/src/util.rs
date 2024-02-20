use super::block::StoredBlock;
use anyhow::{bail, Result};
use std::collections::BTreeMap;

pub(crate) fn verify_dag(blocks: &[StoredBlock]) -> Result<()> {
    if blocks.is_empty() {
        bail!("No blocks is not a meaningful DAG");
    }
    if blocks.len() == 1 {
        if blocks[0].links.is_empty() {
            return Ok(());
        }
        bail!("Given only root of DAG, no children");
    } else if blocks.iter().all(|b| b.links.is_empty()) {
        bail!("No root found");
    }
    let mut counts: BTreeMap<&str, (u16, u16)> = BTreeMap::new();
    for block in blocks {
        block.validate()?;
        counts.entry(block.cid.as_str()).or_default().0 += 1;
        for link in &block.links {
            counts.entry(link.as_str()).or_default().1 += 1;
        }
    }
    let mut root = "";
    for (cid, (h,n)) in counts {
        if n > h {
            bail!("Missing block: {cid}");
        }
        if h == 1 && n == 0 {
            if root.is_empty() {
                root = cid;
            } else if root < cid {
                bail!("Multiple roots! {root} {cid}");
            } else {
                bail!("Multiple roots! {cid} {root}");
            }
        } else if h > n {
            bail!("Too many copies of {cid}");
        }
    }
    if root.is_empty() {
        bail!("DAG is actually DG (cycle detected)");
    }
    Ok(())
}

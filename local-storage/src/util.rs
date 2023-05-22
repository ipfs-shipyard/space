use super::block::StoredBlock;
use anyhow::{bail, Result};

pub(crate) fn verify_dag(blocks: &[StoredBlock]) -> Result<()> {
    use std::collections::{HashMap, HashSet, VecDeque};

    // Map of CID -> linked (1) or not (ud)
    let mut indegree: HashMap<&str, usize> = HashMap::new();
    // List of root blocks
    let mut queue = VecDeque::new();
    let mut visited = HashSet::<&str>::new();
    let mut mp: HashMap<&str, &StoredBlock> = HashMap::new();
    let mut result = vec![];

    blocks.iter().for_each(|block| {
        indegree.insert(&block.cid, 0);
        mp.insert(&block.cid, block);
    });

    // compute in-degree
    for block in blocks.iter() {
        // For each block, examine which CIDs are linked to
        for link in block.links.iter() {
            let link_cid = link.as_str();
            // If the linked CID exists in our collection of blocks, set that CID's degree
            if let Some(degree) = indegree.get_mut(&link_cid) {
                *degree = 1;
            } else {
                bail!("Links do not match blocks");
            }
        }
    }

    // Find roots
    blocks.iter().for_each(|block| {
        if let Some(degree) = indegree.get(&block.cid.as_str()) {
            // For each block, if it is *not* linked, and has not been visited, put in root queue
            // visited has not been touched at this point, so does that check do anything?
            if degree == &0 && !visited.contains(&block.cid.as_str()) {
                queue.push_back(block);
            }
        }
    });

    // DAG should have exactly 1 root
    if queue.len() != 1 {
        bail!("No root found");
    }

    // While there are roots to examine
    while !queue.is_empty() {
        // this is a safe unwrap as the queue is not empty
        let node: &StoredBlock = queue.pop_front().unwrap();
        // ignore a visited node
        if visited.contains(&node.cid.as_str()) {
            continue;
        }

        // validate the root node
        node.validate()?;

        // collect root node in result
        result.push(node);
        // mark root node as visited
        visited.insert(&node.cid);
        // walk the root links
        for link in node.links.iter() {
            let cid = link.as_str();
            // Grab the degree of the linked cid
            if let Some(degree) = indegree.get_mut(&cid) {
                // mark the degree as seen
                *degree -= 1;
                // push a block with in-degree 0 to the queue
                if *degree == 0 {
                    // safe unwrap
                    let block = *mp.get(&cid).unwrap();
                    queue.push_back(block);
                }
            }
        }
    }

    if result.len() != blocks.len() {
        bail!("graph is cyclic");
    }
    Ok(())
}

use crate::{block::StoredBlock, error::StorageError};

use anyhow::{bail, Result};
use rusqlite::Connection;

pub trait StorageProvider {
    // Import a stored block
    fn import_block(&self, block: &StoredBlock) -> Result<()>;
    // Requests a list of CIDs currently available in storage
    fn get_available_cids(&self) -> Result<Vec<String>>;
    // Requests the block associated with the given CID
    fn get_block_by_cid(&self, cid: &str) -> Result<StoredBlock>;
    // Requests the links associated with the given CID
    fn get_links_by_cid(&self, cid: &str) -> Result<Vec<String>>;
    fn list_available_dags(&self) -> Result<Vec<(String, String)>>;
    // Attaches filename to dag
    fn name_dag(&self, cid: &str, file_name: &str) -> Result<()>;
    fn get_missing_cid_blocks(&self, cid: &str) -> Result<Vec<String>>;
    fn get_dag_blocks_by_window(
        &self,
        cid: &str,
        offset: u32,
        window_size: u32,
    ) -> Result<Vec<StoredBlock>>;
    fn get_all_dag_cids(&self, cid: &str) -> Result<Vec<String>>;
    fn get_all_dag_blocks(&self, cid: &str) -> Result<Vec<StoredBlock>>;
    fn get_all_blocks_under_cid(&self, cid: &str) -> Result<Vec<StoredBlock>>;
}

pub struct SqliteStorageProvider {
    conn: Box<Connection>,
}

impl SqliteStorageProvider {
    pub fn new(db_path: &str) -> Result<Self> {
        Ok(SqliteStorageProvider {
            conn: Box::new(Connection::open(db_path)?),
        })
    }

    pub fn setup(&self) -> Result<()> {
        // Create blocks table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS blocks (
                id INTEGER PRIMARY KEY,
                cid TEXT NOT NULL,
                filename TEXT,
                data BLOB
            )",
            (),
        )?;

        // Create links table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS links(
                id INTEGER PRIMARY KEY,
                root_cid TEXT,
                block_cid TEXT NOT NULL,
                block_id INTEGER,
                FOREIGN KEY (block_id) REFERENCES blocks (id)
            )",
            (),
        )?;

        // Create indices
        self.conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS blocks_cid on blocks(cid)",
            (),
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS links_root_cid on links(root_cid)",
            (),
        )?;
        self.conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS links_block_cid on links(block_cid)",
            (),
        )?;

        Ok(())
    }
}

impl StorageProvider for SqliteStorageProvider {
    fn import_block(&self, block: &StoredBlock) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO blocks (cid, data, filename) VALUES (?1, ?2, ?3)",
            (&block.cid, &block.data, &block.filename),
        )?;
        // TODO: Should we have another indicator for root blocks that isn't just the number of links?
        // TODO: This logic should probably get pulled up and split into two parts:
        // 1. import_block - Handles importing block into block store
        // 2. import_block_links - Handles correctly updating links store to account for block
        // If root block with links, then insert links
        if !block.links.is_empty() {
            for link_cid in &block.links {
                let mut maybe_block_id = None;
                if let Ok(block_id) = self.conn.query_row(
                    "SELECT id FROM blocks b
                    WHERE cid == (?1)",
                    [link_cid],
                    |row| {
                        let id: u32 = row.get(0)?;
                        Ok(id)
                    },
                ) {
                    maybe_block_id = Some(block_id);
                }

                self.conn.execute(
                    "INSERT OR IGNORE INTO links (root_cid, block_cid, block_id) VALUES(?1, ?2, ?3)",
                    (&block.cid, link_cid, maybe_block_id),
                )?;
            }
        } else {
            // If child block w/out links...then check if the ID should be inserted into the links table
            self.conn.execute(
                "UPDATE links SET block_id = (SELECT id from blocks WHERE cid = ?1) WHERE block_cid = ?2",
                (&block.cid, &block.cid),
            )?;
        }
        Ok(())
    }

    fn get_available_cids(&self) -> Result<Vec<String>> {
        let cids: Vec<String> = self
            .conn
            .prepare("SELECT cid FROM blocks")?
            .query_map([], |row| row.get(0))?
            // TODO: Correctly catch and log/handle errors here
            .filter_map(|cid| cid.ok())
            .collect();
        Ok(cids)
    }

    fn get_links_by_cid(&self, cid: &str) -> Result<Vec<String>> {
        let links: Vec<String> = self
            .conn
            .prepare("SELECT block_cid FROM links WHERE root_cid == (?1)")?
            .query_map([cid], |row| {
                let cid_str: String = row.get(0)?;
                Ok(cid_str)
            })?
            // TODO: Correctly catch/log/handle errors here
            .filter_map(|cid| cid.ok())
            .collect();
        Ok(links)
    }

    fn get_block_by_cid(&self, cid: &str) -> Result<StoredBlock> {
        match self.conn.query_row(
            "SELECT cid, data, filename FROM blocks b
            WHERE cid == (?1)",
            [&cid],
            |row| {
                let cid_str: String = row.get(0)?;
                let data: Vec<u8> = row.get(1)?;
                let filename: Option<String> = row.get(2).ok();
                Ok(StoredBlock {
                    cid: cid_str,
                    data,
                    links: vec![],
                    filename,
                })
            },
        ) {
            Ok(mut block) => {
                block.links = self.get_links_by_cid(cid)?;
                Ok(block)
            }
            Err(e) => bail!(StorageError::BlockNotFound(cid.to_string(), e.to_string())),
        }
    }

    fn list_available_dags(&self) -> Result<Vec<(String, String)>> {
        let roots = self
            .conn
            .prepare("SELECT DISTINCT cid, filename FROM blocks")?
            .query_map([], |row| {
                let cid_str: String = row.get(0)?;
                let filename_str: String = row.get(1)?;
                Ok((cid_str, filename_str))
            })?
            // TODO: Correctly catch/log/handle errors here
            .filter_map(|cid| cid.ok())
            .collect();
        Ok(roots)
    }

    fn name_dag(&self, cid: &str, file_name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE blocks SET filename = ?1 WHERE cid = ?2",
            (file_name, cid),
        )?;
        Ok(())
    }

    fn get_missing_cid_blocks(&self, cid: &str) -> Result<Vec<String>> {
        // First get all block cid+id associated with root cid
        let blocks: Vec<(String, Option<i32>)> = self
            .conn
            .prepare("SELECT block_cid, block_id FROM links WHERE root_cid == (?1)")?
            .query_map([cid], |row| {
                let block_cid: String = row.get(0)?;
                let block_id: Option<i32> = row.get(1)?;
                Ok((block_cid, block_id))
            })?
            // TODO: Correctly catch/log/handle errors here
            .filter_map(|cid| cid.ok())
            .collect();

        if blocks.is_empty() {
            // If we found no child blocks then be sure the block itself exists
            if self.get_block_by_cid(cid).is_err() {
                bail!("No links or block for CID {cid} found. Root block may be missing.")
            }
        }
        // Then filter by those that are missing a block_id
        let cids: Vec<String> = blocks
            .iter()
            .filter_map(|(cid, id)| match id {
                Some(_) => None,
                None => Some(cid.to_owned()),
            })
            .collect();
        Ok(cids)
    }

    fn get_dag_blocks_by_window(
        &self,
        cid: &str,
        offset: u32,
        window_size: u32,
    ) -> Result<Vec<StoredBlock>> {
        let blocks: Vec<StoredBlock> = self
            .conn
            .prepare(
                "
            WITH RECURSIVE cids(x,y,z) AS (
                SELECT cid,data,filename FROM blocks WHERE cid = (?1)
                UNION
                SELECT cid,data,filename FROM blocks b 
                    INNER JOIN links l ON b.cid==l.block_cid 
                    INNER JOIN cids ON (root_cid=x)
            )
            SELECT x,y,z FROM cids
            LIMIT (?2) OFFSET (?3);
            ",
            )?
            .query_map(
                [cid, &format!("{window_size}"), &format!("{offset}")],
                |row| {
                    let cid_str: String = row.get(0)?;
                    let data: Vec<u8> = row.get(1)?;
                    let filename: Option<String> = row.get(2).ok();
                    let links = match self.get_links_by_cid(&cid_str) {
                        Ok(links) => links,
                        Err(_) => vec![],
                    };
                    Ok(StoredBlock {
                        cid: cid_str,
                        data,
                        links,
                        filename,
                    })
                },
            )?
            .filter_map(|b| b.ok())
            .collect();

        Ok(blocks)
    }

    fn get_all_dag_cids(&self, cid: &str) -> Result<Vec<String>> {
        let cids: Vec<String> = self
            .conn
            .prepare(
                "
                WITH RECURSIVE cids(x) AS (
                    VALUES(?1)
                    UNION
                    SELECT block_cid FROM links JOIN cids ON root_cid=x
                )
                SELECT x FROM cids;
            ",
            )?
            .query_map([cid], |row| {
                let cid_str: String = row.get(0)?;
                Ok(cid_str)
            })?
            .filter_map(|b| b.ok())
            .collect();

        Ok(cids)
    }

    fn get_all_dag_blocks(&self, cid: &str) -> Result<Vec<StoredBlock>> {
        let blocks: Vec<StoredBlock> = self
            .conn
            .prepare(
                "
            WITH RECURSIVE cids(x,y,z) AS (
                SELECT cid,data,filename FROM blocks WHERE cid = (?1)
                UNION
                SELECT cid,data,filename FROM blocks b 
                    INNER JOIN links l ON b.cid==l.block_cid 
                    INNER JOIN cids ON (root_cid=x)
            )
            SELECT x,y,z FROM cids
            ",
            )?
            .query_map([cid], |row| {
                let cid_str: String = row.get(0)?;
                let data: Vec<u8> = row.get(1)?;
                let filename: Option<String> = row.get(2).ok();
                let links = match self.get_links_by_cid(&cid_str) {
                    Ok(links) => links,
                    Err(_) => vec![],
                };
                Ok(StoredBlock {
                    cid: cid_str,
                    data,
                    links,
                    filename,
                })
            })?
            .filter_map(|b| b.ok())
            .collect();

        Ok(blocks)
    }

    fn get_all_blocks_under_cid(&self, cid: &str) -> Result<Vec<StoredBlock>> {
        let blocks: Vec<StoredBlock> = self
            .conn
            .prepare(
                "
            WITH RECURSIVE cids(x,y,z) AS (
                SELECT cid,data,filename FROM blocks WHERE cid = (?1)
                UNION
                SELECT cid,data,filename FROM blocks b 
                    INNER JOIN links l ON b.cid==l.block_cid 
                    INNER JOIN cids ON (root_cid=x)
            )
            SELECT x,y,z FROM cids
            ",
            )?
            .query_map([cid], |row| {
                let cid_str: String = row.get(0)?;
                let data: Vec<u8> = row.get(1)?;
                let filename: Option<String> = row.get(2).ok();
                let links = match self.get_links_by_cid(&cid_str) {
                    Ok(links) => links,
                    Err(_) => vec![],
                };
                Ok(StoredBlock {
                    cid: cid_str,
                    data,
                    links,
                    filename,
                })
            })?
            .filter_map(|b| b.ok())
            .collect();

        Ok(blocks)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use assert_fs::{fixture::PathChild, TempDir};
    use cid::multihash::MultihashDigest;
    use cid::Cid;

    struct TestHarness {
        provider: SqliteStorageProvider,
        _db_dir: TempDir,
    }

    impl TestHarness {
        pub fn new() -> Self {
            let db_dir = TempDir::new().unwrap();
            let db_path = db_dir.child("storage.db");
            let provider = SqliteStorageProvider::new(db_path.path().to_str().unwrap()).unwrap();
            provider.setup().unwrap();
            TestHarness {
                provider,
                _db_dir: db_dir,
            }
        }
    }

    #[test]
    pub fn test_create_sqlite_provider() {
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.child("storage.db");
        let provider = SqliteStorageProvider::new(db_path.to_str().unwrap()).unwrap();
        provider.setup().unwrap();
    }

    #[test]
    pub fn test_import_one_block() {
        let harness = TestHarness::new();

        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(b"00"));
        let cid_str = cid.to_string();
        let block = StoredBlock {
            cid: cid_str.to_string(),
            data: b"1010101".to_vec(),
            links: vec![],
            filename: None,
        };

        harness.provider.import_block(&block).unwrap();
        let cids_list = harness.provider.get_available_cids().unwrap();
        assert_eq!(cids_list.len(), 1);
        assert_eq!(cids_list.first().unwrap(), &cid_str);
    }

    #[test]
    pub fn test_import_three_blocks() {
        use std::collections::HashSet;
        use std::iter::FromIterator;

        let harness = TestHarness::new();

        let seeds = vec![b"00", b"11", b"22"];
        let cids: Vec<String> = seeds
            .iter()
            .map(|s| {
                Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(s.as_slice())).to_string()
            })
            .collect();

        cids.iter().for_each(|c| {
            let block = StoredBlock {
                cid: c.to_string(),
                data: b"123412341234".to_vec(),
                links: vec![],
                filename: None,
            };
            harness.provider.import_block(&block).unwrap()
        });

        let cids_list = harness.provider.get_available_cids().unwrap();
        assert_eq!(cids_list.len(), 3);
        let set_one: HashSet<&String> = HashSet::from_iter(cids.iter());
        let set_two: HashSet<&String> = HashSet::from_iter(cids_list.iter());
        assert_eq!(set_one, set_two);
    }

    #[test]
    pub fn test_import_then_get_block() {
        let harness = TestHarness::new();

        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(b"00"));

        let block = StoredBlock {
            cid: cid.to_string(),
            data: b"1010101".to_vec(),
            links: vec![],
            filename: None,
        };

        harness.provider.import_block(&block).unwrap();
        let cids_list = harness.provider.get_available_cids().unwrap();
        let cid_str = cids_list.first().unwrap();

        let fetched_block = harness.provider.get_block_by_cid(cid_str).unwrap();
        assert_eq!(block, fetched_block);
    }

    #[test]
    pub fn test_import_then_get_root_block() {
        let harness = TestHarness::new();

        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(b"00"));
        let block_cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(b"11"));

        let block = StoredBlock {
            cid: cid.to_string(),
            data: b"1010101".to_vec(),
            links: vec![block_cid.to_string()],
            filename: None,
        };

        harness.provider.import_block(&block).unwrap();
        let cids_list = harness.provider.get_available_cids().unwrap();
        let cid_str = cids_list.first().unwrap();

        let fetched_block = harness.provider.get_block_by_cid(cid_str).unwrap();
        assert_eq!(block, fetched_block);
    }

    #[test]
    pub fn test_verify_detect_missing_blocks() {
        let harness = TestHarness::new();

        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(b"00"));
        let block_cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(b"11"));

        let block = StoredBlock {
            cid: cid.to_string(),
            data: vec![],
            links: vec![block_cid.to_string()],
            filename: None,
        };

        harness.provider.import_block(&block).unwrap();
        let cids_list = harness.provider.get_available_cids().unwrap();
        let cid_str = cids_list.first().unwrap();

        let fetched_block = harness.provider.get_block_by_cid(cid_str).unwrap();
        assert_eq!(block, fetched_block);
        assert_eq!(harness.provider.get_links_by_cid(cid_str).unwrap().len(), 1);
        assert_eq!(
            harness
                .provider
                .get_missing_cid_blocks(cid_str)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    pub fn test_verify_detect_no_missing_blocks() {
        let harness = TestHarness::new();

        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(b"00"));
        let cid_str = cid.to_string();
        let block_cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(b"11"));

        let block = StoredBlock {
            cid: cid_str.to_string(),
            data: vec![],
            links: vec![block_cid.to_string()],
            filename: None,
        };

        let child_block = StoredBlock {
            cid: block_cid.to_string(),
            data: b"101293910101".to_vec(),
            links: vec![],
            filename: None,
        };

        harness.provider.import_block(&block).unwrap();
        harness.provider.import_block(&child_block).unwrap();
        let cids_list = harness.provider.get_available_cids().unwrap();
        assert_eq!(cids_list.len(), 2);

        let fetched_block = harness.provider.get_block_by_cid(&cid_str).unwrap();
        assert_eq!(block, fetched_block);
        assert_eq!(
            harness.provider.get_links_by_cid(&cid_str).unwrap().len(),
            1
        );
        assert_eq!(
            harness
                .provider
                .get_missing_cid_blocks(&cid_str)
                .unwrap()
                .len(),
            0
        );
    }
}

use crate::{block::StoredBlock, error::StorageError, provider::StorageProvider};
use anyhow::{bail, Result};
use cid::Cid;
use log::{debug, info, trace};
use rusqlite::{params_from_iter, Connection};
use std::{path::PathBuf, str::FromStr};

pub struct SqliteStorageProvider {
    conn: Box<Connection>,
}

impl SqliteStorageProvider {
    pub fn new(db_path: &str) -> Result<Self> {
        let mut db_path = PathBuf::from_str(db_path)?;
        loop {
            if db_path.is_dir() {
                db_path = db_path.join("storage.db");
            } else if db_path.exists()
                || db_path.extension().unwrap_or_default().to_str() == Some("db")
            {
                break;
            } else {
                db_path = db_path.join("storage.db");
            }
        }
        let result = SqliteStorageProvider {
            conn: Box::new(Connection::open(db_path)?),
        };
        result.setup()?;
        Ok(result)
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
                sequence INTEGER,
                root_cid TEXT,
                block_cid TEXT NOT NULL,
                block_id INTEGER,
                PRIMARY KEY (sequence, root_cid),
                FOREIGN KEY (block_id) REFERENCES blocks (id)
            )",
            (),
        )?;
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS orphans(cid TEXT PRIMARY KEY)",
            [],
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

        Ok(())
    }

    fn get_blocks_recursive(
        &self,
        cid: &str,
        offset: Option<u32>,
        window_size: Option<u32>,
    ) -> Result<Vec<StoredBlock>> {
        let mut blocks = vec![self.get_block_by_cid(cid)?];
        let mut i = 0;
        while i < blocks.len() {
            let links = blocks[i].links.clone();
            for link in links {
                blocks.push(self.get_block_by_cid(&link)?);
            }
            i += 1;
        }
        let off = offset.unwrap_or(0).try_into()?;
        let siz = window_size.map(|n| n as usize).unwrap_or(blocks.len());
        Ok(blocks.into_iter().skip(off).take(siz).collect())
    }
}

impl StorageProvider for SqliteStorageProvider {
    fn import_block(&mut self, block: &StoredBlock) -> Result<()> {
        if 1 == self.conn.execute(
            "INSERT OR IGNORE INTO blocks (cid, data, filename) VALUES (?1, ?2, ?3)",
            (&block.cid, &block.data, &block.filename),
        )? {
            debug!("Inserted block {block:?}");
        }
        // TODO: Should we have another indicator for root blocks that isn't just the number of links?
        // TODO: This logic should probably get pulled up and split into two parts:
        // 1. import_block - Handles importing block into block store
        // 2. import_block_links - Handles correctly updating links store to account for block
        // If root block with links, then insert links
        if !block.links.is_empty() {
            self.conn
                .execute("DELETE FROM links WHERE root_cid = ?1", [&block.cid])?;
            for (link_sequence, link_cid) in block.links.iter().enumerate() {
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
                    "INSERT OR IGNORE INTO links (sequence, root_cid, block_cid, block_id) VALUES(?1, ?2, ?3, ?4)",
                    (link_sequence, &block.cid, link_cid, maybe_block_id),
                )?;
            }
        } else {
            // If child block w/out links...then check if the ID should be inserted into the links table
            self.conn.execute(
                "UPDATE links SET block_id = (SELECT id from blocks WHERE cid = ?1) WHERE block_cid = ?2",
                (&block.cid, &block.cid),
            )?;
        }
        self.conn
            .execute("DELETE FROM orphans WHERE cid = ?1", [&block.cid])?;
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
            .prepare("SELECT block_cid FROM links WHERE root_cid == (?1) ORDER BY sequence")?
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
        let updated_count = self.conn.execute(
            "UPDATE blocks SET filename = ?1 WHERE cid = ?2",
            (file_name, cid),
        )?;
        if updated_count != 1 {
            bail!("When naming DAG {cid} {file_name}, expected it to hit exactly 1 row, not {updated_count}");
        }
        info!("Named {cid} {file_name}");
        Ok(())
    }

    fn get_missing_cid_blocks(&self, cid: &str) -> Result<Vec<String>> {
        // First get all block cid+id associated with root cid
        let blocks: Vec<(String, Option<i32>)> = self
            .conn
            .prepare(
                "
                WITH RECURSIVE cids(x,y) AS (
                    SELECT cid, id FROM blocks WHERE cid = (?1)
                    UNION
                    SELECT block_cid, block_id FROM links l JOIN cids ON root_cid=x  
                )
                SELECT x,y FROM cids;
            ",
            )?
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
        let blocks = self.get_blocks_recursive(cid, Some(offset), Some(window_size))?;

        Ok(blocks)
    }

    fn get_all_dag_cids(
        &self,
        cid: &str,
        offset: Option<u32>,
        window_size: Option<u32>,
    ) -> Result<Vec<String>> {
        let mut base_query = "
                WITH RECURSIVE cids(x,ignore) AS (
                    VALUES( ?1 , 0)
                    UNION
                    SELECT block_cid , l.sequence FROM links l JOIN cids ON root_cid=x ORDER BY l.sequence
                )
                SELECT x FROM cids
            "
        .to_string();
        let mut params = vec![cid.to_string()];
        if let Some(offset) = offset {
            if let Some(window_size) = window_size {
                base_query.push_str(" LIMIT (?2) OFFSET (?3);");
                params.push(format!("{window_size}"));
                params.push(format!("{offset}"));
            }
        }
        let params = params_from_iter(params.into_iter());
        let cids: Vec<String> = self
            .conn
            .prepare(&base_query)?
            .query_map(params, |row| {
                let cid_str: String = row.get(0)?;
                Ok(cid_str)
            })?
            .filter_map(|b| b.ok())
            .collect();

        Ok(cids)
    }

    fn get_all_dag_blocks(&self, cid: &str) -> Result<Vec<StoredBlock>> {
        self.get_blocks_recursive(cid, None, None)
    }

    fn incremental_gc(&mut self) -> bool {
        trace!("TODO incremental_gc");
        false
    }

    fn has_cid(&self, cid: &Cid) -> bool {
        self.conn
            .query_row(
                "SELECT 1 FROM blocks WHERE cid = ?1",
                [cid.to_string()],
                |_| Ok(()),
            )
            .is_ok()
    }

    fn ack_cid(&self, cid: &Cid) {
        if !self.has_cid(cid) {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO orphans ( cid ) VALUES ( ?1 )",
                    [&cid.to_string()],
                )
                .ok();
        }
    }

    fn get_dangling_cids(&self) -> Result<Vec<Cid>> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT cid FROM orphans")?;
        let rs = stmt.query_map([], |r| r.get::<usize, String>(0))?;
        let mut result = Vec::default();
        for s in rs.flat_map(|rs| rs.ok()) {
            if let Ok(cid) = Cid::try_from(s.as_str()) {
                result.push(cid);
            }
        }
        Ok(result)
    }

    fn get_name(&self, cid: &str) -> Result<String> {
        let result = self.conn.query_row(
            "SELECT MAX(filename) FROM blocks WHERE cid = ?1",
            [cid],
            |r| r.get(0),
        )?;
        Ok(result)
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
        let mut harness = TestHarness::new();

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

        let mut harness = TestHarness::new();

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
        let mut harness = TestHarness::new();

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
        let mut harness = TestHarness::new();

        let cid = Cid::new_v1(0x70, cid::multihash::Code::Sha2_256.digest(b"00"));
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
        let mut harness = TestHarness::new();

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
        let mut harness = TestHarness::new();

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

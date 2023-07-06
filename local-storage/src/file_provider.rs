use crate::{block::StoredBlock, provider::StorageProvider};
use anyhow::Result;
use cid::{multibase, Cid};
use std::io::Read;
use std::{
    fs::{canonicalize, create_dir_all, read_dir, DirEntry, File},
    io::Write,
    path::PathBuf,
};

pub(crate) struct FileStorageProvider {
    dir: PathBuf,
}

impl FileStorageProvider {
    #[allow(dead_code)]
    pub fn new(storage_folder: &str) -> Result<Self> {
        let mut me = Self {
            dir: storage_folder.into(),
        };
        create_dir_all(&me.blocks())?;
        me.dir = canonicalize(storage_folder)?;
        create_dir_all(&me.cids())?;
        create_dir_all(&me.names())?;
        Ok(me)
    }
    fn blocks(&self) -> PathBuf {
        self.dir.join("blocks")
    }
    fn cids(&self) -> PathBuf {
        self.dir.join("cids")
    }
    fn names(&self) -> PathBuf {
        self.dir.join("names")
    }
    fn get_name(&self, cid: &str) -> Result<String> {
        let mut result = String::default();
        File::open(self.names().join(cid))?.read_to_string(&mut result)?;
        Ok(result)
    }
    fn block_path(&self, cid: &Cid) -> PathBuf {
        let mh = cid.hash().to_bytes();
        let hash = multibase::encode(multibase::Base::Base36Lower, mh);
        self.blocks().join(hash)
    }
    fn get_missing(&self, out: &mut Vec<String>, cid: &str) {
        if let Ok(block) = self.get_block_by_cid(cid) {
            for link in block.links {
                self.get_missing(out, link.as_str());
            }
        } else {
            out.push(cid.to_string());
        }
    }
    fn get_blocks(&self, out: &mut Vec<StoredBlock>, cid: &str) -> Result<()> {
        let block = self.get_block_by_cid(cid)?;
        let links = block.links.clone();
        out.push(block);
        for link in links {
            self.get_blocks(out, link.as_str())?;
        }
        Ok(())
    }

    fn find_window(
        &self,
        result: &mut Vec<StoredBlock>,
        curr_cid: &str,
        mut to_skip: u32,
        mut to_fetch: u32,
    ) -> Result<(u32, u32)> {
        let block = self.get_block_by_cid(curr_cid)?;
        if to_skip > 0 {
            to_skip -= 1;
        } else if to_fetch > 0 {
            result.push(block.clone());
            to_fetch -= 1;
        }
        for link in block.links {
            if to_fetch == 0 {
                return Ok((0, 0));
            }
            let (s, f) = self.find_window(result, &link, to_skip, to_fetch)?;
            to_skip = s;
            to_fetch = f;
        }
        Ok((to_skip, to_fetch))
    }
}
fn entry_to_cid_str(r: std::io::Result<DirEntry>) -> Option<String> {
    let e = r.ok()?;
    if e.metadata().ok()?.is_file() {
        Some(e.file_name().to_str()?.to_owned())
    } else {
        None
    }
}
impl StorageProvider for FileStorageProvider {
    fn import_block(&self, block: &StoredBlock) -> anyhow::Result<()> {
        let cid = Cid::try_from(block.cid.as_str())?;
        let block_path = self.block_path(&cid);
        File::create(&block_path)?.write_all(block.data.as_slice())?;
        let mut f = File::create(self.cids().join(&block.cid))?;
        for l in &block.links {
            writeln!(&mut f, "{}", &l)?;
        }
        if let Some(name) = &block.filename {
            self.name_dag(&block.cid, name)?;
        }
        Ok(())
    }

    fn get_available_cids(&self) -> anyhow::Result<Vec<String>> {
        let mut result: Vec<String> = read_dir(self.cids())?
            .filter_map(entry_to_cid_str)
            .collect();
        result.sort();
        Ok(result)
    }

    fn get_block_by_cid(&self, cid_str: &str) -> anyhow::Result<StoredBlock> {
        let mut result = StoredBlock {
            cid: cid_str.to_string(),
            filename: self.get_name(cid_str).ok(),
            data: vec![],
            links: vec![],
        };
        let cid = Cid::try_from(cid_str)?;
        let block_path = self.block_path(&cid);
        File::open(block_path)?.read_to_end(&mut result.data)?;
        result.links = self.get_links_by_cid(cid_str)?;
        Ok(result)
    }

    fn get_links_by_cid(&self, cid: &str) -> anyhow::Result<Vec<String>> {
        let links_path = self.cids().join(cid);
        let result = std::fs::read_to_string(links_path)?
            .lines()
            .map(String::from)
            .collect();
        Ok(result)
    }

    fn list_available_dags(&self) -> anyhow::Result<Vec<(String, String)>> {
        Ok(self
            .get_available_cids()?
            .into_iter()
            .map(|c| {
                let n = self.get_name(c.as_str()).ok().unwrap_or_else(String::new);
                (c, n)
            })
            .collect())
    }

    fn name_dag(&self, cid: &str, file_name: &str) -> anyhow::Result<()> {
        File::create(self.names().join(cid))?.write_all(file_name.as_bytes())?;
        Ok(())
    }

    fn get_missing_cid_blocks(&self, cid: &str) -> anyhow::Result<Vec<String>> {
        let mut result = Vec::new();
        self.get_missing(&mut result, cid);
        Ok(result)
    }

    fn get_dag_blocks_by_window(
        &self,
        cid: &str,
        offset: u32,
        window_size: u32,
    ) -> anyhow::Result<Vec<StoredBlock>> {
        let mut result = Vec::new();
        self.find_window(&mut result, cid, offset, window_size)?;
        Ok(result)
    }

    fn get_all_dag_cids(
        &self,
        cid: &str,
        offset: Option<u32>,
        window_size: Option<u32>,
    ) -> anyhow::Result<Vec<String>> {
        let blocks = self.get_all_dag_blocks(cid)?;
        Ok(blocks
            .into_iter()
            .skip(offset.unwrap_or(0).try_into()?)
            .take(window_size.unwrap_or(0).try_into()?)
            .map(|b| b.cid)
            .collect())
    }

    fn get_all_dag_blocks(&self, cid: &str) -> anyhow::Result<Vec<StoredBlock>> {
        let mut result = Vec::new();
        self.get_blocks(&mut result, cid)?;
        Ok(result)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use assert_fs::TempDir;
    use cid::multihash::MultihashDigest;
    use cid::Cid;

    struct TestHarness {
        provider: FileStorageProvider,
    }
    impl TestHarness {
        pub fn new() -> TestHarness {
            let td = TempDir::new().unwrap().canonicalize().unwrap();
            let dir = td.as_os_str().to_str().unwrap();
            let provider = FileStorageProvider::new(dir).unwrap();
            Self { provider }
        }
        pub fn import_hi(&mut self, deep: bool) -> StoredBlock {
            //The SQL Provider version of this test uses an invalid node (doesn't even parse) and relies on the implementation returning whatever it was told the links are, regardless of what they actually are (not verifying deeply)
            //The File Provider currently relies on the links being real, so hear we have a minimal example: the data is 2 bytes, the text "hi", chunked with size 1 and using raw leaves
            let h = Cid::try_from("bafkreifkvfacmzhruqpub254klezspvwnlvtmzqcswh57krihny6mtnrem")
                .unwrap();
            let i = Cid::try_from("bafkreig6punxegq6ayzlptye5x2qgleoz75j7gqijeqvfojg6gs2pz3f24")
                .unwrap();
            let hi = Cid::try_from("bafybeibhdee56vnqurkkk53wsfik3nkkgteuoi5nsarmbtsvi5wrxkopki")
                .unwrap();
            let block_bytes: &[u8] = &[
                0x12u8, 0x2a, 0x0a, 0x24, 0x01, 0x55, 0x12, 0x20, 0xaa, 0xa9, 0x40, 0x26, 0x64,
                0xf1, 0xa4, 0x1f, 0x40, 0xeb, 0xbc, 0x52, 0xc9, 0x99, 0x3e, 0xb6, 0x6a, 0xeb, 0x36,
                0x66, 0x02, 0x95, 0x8f, 0xdf, 0xaa, 0x28, 0x3b, 0x71, 0xe6, 0x4d, 0xb1, 0x23, 0x12,
                0x00, 0x18, 0x01, 0x12, 0x2a, 0x0a, 0x24, 0x01, 0x55, 0x12, 0x20, 0xde, 0x7d, 0x1b,
                0x72, 0x1a, 0x1e, 0x06, 0x32, 0xb7, 0xcf, 0x04, 0xed, 0xf5, 0x03, 0x2c, 0x8e, 0xcf,
                0xfa, 0x9f, 0x9a, 0x08, 0x49, 0x21, 0x52, 0xb9, 0x26, 0xf1, 0xa5, 0xa7, 0xe7, 0x65,
                0xd7, 0x12, 0x00, 0x18, 0x01, 0x0a, 0x08, 0x08, 0x02, 0x18, 0x02, 0x20, 0x01, 0x20,
                0x01,
            ];
            let root = StoredBlock {
                cid: hi.to_string(),
                data: block_bytes.to_vec(),
                links: vec![h.to_string(), i.to_string()],
                filename: None,
            };
            self.provider.import_block(&root).unwrap();
            if deep {
                let block = StoredBlock {
                    cid: h.to_string(),
                    data: vec![0x68], //'h'
                    links: vec![],
                    filename: None,
                };
                self.provider.import_block(&block).unwrap();
                let block = StoredBlock {
                    cid: i.to_string(),
                    data: vec![0x69], //'i'
                    links: vec![],
                    filename: None,
                };
                self.provider.import_block(&block).unwrap();
            }
            root
        }
    }

    #[test]
    pub fn test_create_file_provider() {
        TestHarness::new();
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

        harness
            .provider
            .import_block(&block)
            .expect("Importing a block returned Err");
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
        let mut harness = TestHarness::new();
        let root = harness.import_hi(false);

        let cids_list = harness.provider.get_available_cids().unwrap();
        let cid_str = cids_list.first().unwrap();

        let fetched_block = harness.provider.get_block_by_cid(cid_str).unwrap();
        assert_eq!(root, fetched_block);
    }

    #[test]
    pub fn test_verify_detect_missing_blocks() {
        let mut harness = TestHarness::new();

        let root = harness.import_hi(false);
        let cids_list = harness.provider.get_available_cids().unwrap();
        let cid_str = cids_list.first().unwrap();

        let fetched_block = harness.provider.get_block_by_cid(cid_str).unwrap();
        assert_eq!(root, fetched_block);
        let links = harness.provider.get_links_by_cid(cid_str).unwrap();
        let missing_links = harness.provider.get_missing_cid_blocks(cid_str).unwrap();
        assert_eq!(links.len(), 2);
        assert_eq!(missing_links.len(), 2);
        assert_eq!(links, missing_links);
    }

    #[test]
    pub fn test_verify_detect_no_missing_blocks() {
        let mut harness = TestHarness::new();

        let root = harness.import_hi(true);
        let cid_str = root.cid.as_str();

        let fetched_block = harness.provider.get_block_by_cid(cid_str).unwrap();
        assert_eq!(root, fetched_block);
        let links = harness.provider.get_links_by_cid(cid_str).unwrap();
        let missing_links = harness.provider.get_missing_cid_blocks(cid_str).unwrap();
        assert_eq!(links.len(), 2);
        assert_eq!(missing_links.len(), 0);
    }
}

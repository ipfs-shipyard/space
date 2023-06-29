use anyhow::Result;
use bytes::Bytes;
use chrono::{DateTime, offset::Utc};
use cid::Cid;
use messages::TransmissionBlock;
use std::{cmp::Ordering, collections::{BTreeMap, HashSet}};
use super::kubo_api::{Key, KuboApi, KuboError};
use tracing::{debug, info, warn, error};
use ipfs_unixfs::{unixfs, unixfs::{dag_pb, unixfs_pb, UnixfsNode}};

const PREFERRED_KEYS_IN_ORDER: &[&str] = &["hyphae", "myceli", "space", "", "self"];

pub(crate) struct Indexer<'a> {
    kubo: &'a KuboApi,
    // myceli: &'a MyceliApi,
    key: Option<Key>,
    html: String,
    main: Index,
    resolved: Option<String>,
    dir: Option<ipfs_unixfs::Block>,
    name_target: Option<String>,
    index_count: usize,
}

type When = DateTime<Utc>;

#[derive(Debug, Clone, Ord, Eq, PartialEq, Default)]
struct File {
    name: String,
    cid: String,
    when: When,
}

struct Index {
    files: BTreeMap<String, File>,
    arch: Option<Box<Index>>,
}

const PARSED: &str = "Already parsed the HTML";
const DATA_LINE_START: &str = "<!-- ";

impl<'a> Indexer<'a> {
    // pub fn new(kubo: &'a KuboApi, myceli: &'a MyceliApi) -> Self {
    pub fn new(kubo: &'a KuboApi) -> Self {
        Self { kubo, key: None, html: String::new(), main: Index::default(), resolved: None, name_target: None, dir: None, index_count: 0 }
    }
    pub fn step(&mut self, files: &BTreeMap<String, TransmissionBlock>) -> Result<bool> {
        if self.add_files(files) {
            info!("Added some new file(s).");
            Ok(true)
        } else if self.key.is_none() {
            let known = self.kubo.list_keys()?.keys;
            debug!("All keys Kubo currently knows: {:?}", &known);
            for pref in PREFERRED_KEYS_IN_ORDER {
                self.key = known.iter().find(|k| k.name == *pref).map(|k| k.clone());
                if self.key.is_some() {
                    info!("Will be publishing to IPNS name '{}'", & pref);
                    break;
                } else {
                    warn!("Kubo knows of no key by the name '{}', so it will not be used.", & pref);
                }
            }
            if self.key.is_none() {
                self.key = known.into_iter().next();
                if self.key.is_some() {
                    warn!("Found no preferred keys, so using one arbitrarily: {:?}", & self.key);
                }
            }
            Ok(self.key.is_some())
        } else if self.resolved.is_none() {
            match self.kubo.resolve_name(&self.key.clone().unwrap().id) {
                Ok(ipfs_path) => { self.resolved = Some(ipfs_path) }
                Err(KuboError::NoSuchName(_)) => {
                    info!("Our IPNS name does not currently resolve to anything. Presumably we're just getting started from scratch.");
                    self.resolved = Some(String::new());
                }
                Err(e) => error!("Error resolving IPNS name: {:?}", & e),
            }
            Ok(true)
        } else if self.html.is_empty() {
            self.fetch_html()?;
            Ok(true)
        } else if self.html != PARSED {
            self.parse_html(files);
            Ok(true)
        } else if let Some(d) = &self.dir {
            self.kubo.put_block(d.cid().to_string().as_str(), &TransmissionBlock {
                data: d.data().to_vec(),
                cid: vec![],
                links: vec![],
                filename: None,
            })?;
            info!("Uploaded the directory node {}", &d.cid());
            self.name_target = Some(d.cid().to_string());
            self.dir = None;
            Ok(true)
        } else if let (Some(target), Some(key)) = (&self.name_target, &self.key) {
            let path = "/ipfs/".to_string() + target;
            debug!("About to attempt to publish...");
            self.kubo.publish(key.name.as_str(), &path)?;
            info!("Published. See results at http://localhost:8080/ipns/{}", key.id);
            self.name_target = None;
            Ok(true)
        } else if self.index_count == files.len() {
            debug!("Waiting for new files to arrive.");
            Ok(false)
        } else if let Ok([index, dir]) = self.main.build() {
            info!("Will upload index.html {} now, directory {} later", index.cid().to_string(), dir.cid().to_string());
            self.kubo.put_block(index.cid().to_string().as_str(), &TransmissionBlock {
                data: index.data().to_vec(),
                cid: vec![],
                links: vec![],
                filename: None,
            })?;
            self.dir = Some(dir);
            self.index_count = files.len();
            Ok(true)
        } else {
            warn!("troubles");
            Ok(false)
        }
    }
    fn add_files(&mut self, files: &BTreeMap<String, TransmissionBlock>) -> bool {
        if self.html != PARSED {
            debug!("We don't yet have an 'old' index to check for files with existing timestamps.");
            return false;
        }
        let when = Utc::now();
        let mut result = false;
        for (cid, tblock) in files {
            if self.main.files.contains_key(cid) {
                debug!("{} already included in main index", cid);
            } else if self.main.is_archived(cid) {
                debug!("{} already included in archive index", cid);
            } else if let Some(name) = &tblock.filename {
                let file = File { name: name.clone(), cid: cid.clone(), when };
                info!("Encountered new file: {:?} aka {:?}", & file, &tblock);
                self.main.files.insert(cid.clone(), file);
                result = true;
            } else {
                debug!("Not indexing unnamed chunk: {}", & cid);
            }
        }
        result
    }
    fn fetch_html(&mut self) -> Result<()> {
        if let (Some(path), Some(key)) = (&self.resolved, &self.key) {
            if path.is_empty() {
                info!("We have no HTML to parse.");
                self.html = PARSED.to_string();
            } else {
                let bytes = self.kubo.get(format!("/ipns/{}/index.html", key.id).as_str())?;
                info!("Fetched {} bytes of index.html", bytes.len());
                self.html = String::from_utf8(bytes)?;
            }
            Ok(())
        } else {
            unreachable!()
        }
    }
    fn parse_html(&mut self, files: &BTreeMap<String, TransmissionBlock>) {
        for line in self.html.split("\n") {
            if line.starts_with(DATA_LINE_START) {
                let mut toks = line.split_whitespace();
                toks.next();//discard open comment
                let mut f = File::default();
                if let Some(w) = toks.next().and_then(|s| DateTime::parse_from_rfc3339(s).ok()) {
                    f.when = w.into();
                } else {
                    warn!("Trouble parsing timestamp.");
                    continue;
                }
                if let Some(cid) = toks.next() {
                    f.cid = cid.to_owned();
                } else {
                    warn!("No CID in data line?");
                    continue;
                }
                if let Some(name) = toks.next() {
                    f.name = name.to_owned();
                } else {
                    warn!("No filename in data line");
                    continue;
                }
                if files.contains_key(&f.cid) {
                    info!("Re-found old file={:?}", &f);
                    self.main.files.insert(f.cid.clone(), f);
                } else {
                    info!("Missing old file={:?}", &f);
                    if self.main.arch.is_none() {
                        self.main.arch = Some(Box::new(Index::default()));
                    }
                    self.main.arch.as_mut().unwrap().files.insert(f.cid.clone(), f);
                }
            }
        }
        self.html = PARSED.to_string();
    }
}

impl Index {
    fn is_archived(&self, cid: &String) -> bool {
        if let Some(a) = &self.arch {
            (*a).files.contains_key(cid) || (*a).is_archived(cid)
        } else {
            false
        }
    }
    fn build(&self) -> Result<[ipfs_unixfs::Block; 2]> {
        let mut html = "<html><title>Space Files</title><body><table border=1><tr><th>Name</th><th>Import Time</th></tr>\n".to_string();
        let mut links = Vec::new();
        let mut files: Vec<File> = self.files.values().map(|f| (*f).clone()).collect();
        files.sort();
        let mut taken: HashSet<String> = HashSet::new();
        taken.insert("archive".to_string());
        taken.insert("index.html".to_string());
        let mut i = 0;
        for f in files {
            if html.len() > 1_000_000 {
                warn!("index.html limited to fit in a single UnixFS node.");
                break;
            }
            let mut real = f.name.clone();
            while !taken.insert(real.clone()) {
                real = i.to_string() + real.as_str();
                i += 1;
            }
            let cid = Cid::try_from(f.cid.as_str());
            if cid.is_err() {
                continue;
            }
            let cid = cid?;
            info!("Link: {}={}={}", &f.name, &real, &f.cid);
            links.push(dag_pb::PbLink {
                hash: Some(cid.to_bytes()),
                name: Some(real.clone()),
                tsize: None,
            });
            let when_str = f.when.to_rfc3339();
            html.push_str(DATA_LINE_START);
            html.push_str(&when_str);
            html.push(' ');
            html.push_str(&cid.to_string());
            html.push(' ');
            html.push_str(&f.name);
            html.push_str(" -->\n");

            html.push_str("<tr><td><a href='");
            html.push_str(&real);
            html.push_str("'>");
            html.push_str(&f.name);
            html.push_str("</a></td><td>");
            html.push_str(&when_str);
            html.push_str("</td></tr>\n");
        }
        html.push_str("</table></body></html>");
        let html_bytes = Bytes::from(html);
        let byte_count = html_bytes.len() as u64;
        let node = UnixfsNode::Raw(html_bytes);
        let index_block = node.encode()?;
        links.push(dag_pb::PbLink {
            hash: Some(index_block.cid().to_bytes()),
            name: Some("index.html".to_string()),
            tsize: Some(byte_count),
        });
        let inner = unixfs_pb::Data {
            r#type: unixfs::DataType::Directory as i32,
            ..Default::default()
        };
        let outer = ipfs_unixfs::builder::encode_unixfs_pb(&inner, links)?;
        let node = UnixfsNode::Directory(unixfs::Node { outer, inner });
        Ok([index_block, node.encode()?])
    }
}

impl Default for Index {
    fn default() -> Self {
        Self {
            files: BTreeMap::new(),
            arch: None,
        }
    }
}

impl PartialOrd<File> for File {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        for o in &[self.when.cmp(&other.when).reverse(), self.name.cmp(&other.name)] {
            if !o.is_eq() {
                return Some(*o);
            }
        }
        Some(self.cid.cmp(&other.cid))
    }
}

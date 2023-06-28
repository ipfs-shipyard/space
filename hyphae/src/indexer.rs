use super::kubo_api::{Key, KuboApi};
use anyhow::Result;
use tracing::{debug, info, warn};

const PREFERRED_KEYS_IN_ORDER : &[&str] = &["hyphae", "myceli", "space", "", "self"];

pub(crate) struct Indexer<'a> {
    kubo: &'a KuboApi,
    // myceli: &'a MyceliApi,
    key: Option<Key>,
    html: String,
}

impl<'a> Indexer<'a> {
    // pub fn new(kubo: &'a KuboApi, myceli: &'a MyceliApi) -> Self {
    pub fn new(kubo: &'a KuboApi) -> Self {
        Self{kubo, key: None, html: String::new()}
    }
    pub fn step(&mut self) -> Result<bool> {
        if self.key.is_none() {
            let known = self.kubo.list_keys()?.keys;
            debug!("All keys Kubo currently knows: {:?}", &known);
            for pref in PREFERRED_KEYS_IN_ORDER {
                self.key = known.iter().find(|k| k.name == *pref).map(|k|k.clone());
                if self.key.is_some()  {
                    info!("Will be publishing to IPNS name '{}'", &pref);
                } else {
                    warn!("Kubo knows of no key by the name '{}', so it will not be used.", &pref);
                }
            }
            if self.key.is_none() {
                self.key = known.into_iter().next();
                if self.key.is_some() {
                    warn!("Found no preferred keys, so using one arbitrarily: {:?}", &self.key);
                }
            }
            Ok(self.key.is_some())
        } else {
            info!("TODO");
            Ok(false)
        }
    }
}
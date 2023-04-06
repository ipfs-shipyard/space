use anyhow::Result;
use messages::TransmissionBlock;
use reqwest::blocking::multipart;
use reqwest::blocking::Client;
use serde_json::{Deserializer, Value};
use std::collections::HashSet;
use std::time::Duration;
use tracing::{info, warn};

use crate::DAG_PB_CID_PREFIX;

pub struct KuboApi {
    address: String,
    client: Client,
}

impl KuboApi {
    pub fn new(address: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(5000))
            .build()
            .expect("Failed to build reqwest client");
        KuboApi {
            address: format!("http://{}/api/v0", address),
            client,
        }
    }

    pub fn check_alive(&self) -> bool {
        let version_url = format!("{}/version", self.address);
        match self
            .client
            .post(version_url)
            .send()
            .and_then(|resp| resp.json::<Value>())
        {
            Ok(resp) => {
                info!("Found Kubo version {}", resp["Version"]);
                true
            }
            Err(e) => {
                warn!("Could not contact Kubo at this time: {e}");
                false
            }
        }
    }

    pub fn get_local_blocks(&self) -> Result<HashSet<String>> {
        let local_refs_addr = format!("{}/refs/local", self.address);
        let resp: String = self.client.post(local_refs_addr).send()?.text()?;
        let de = Deserializer::from_str(&resp);
        let mut de_stream = de.into_iter::<Value>();
        let mut cids = HashSet::new();
        while let Some(Ok(next)) = de_stream.next() {
            if let Some(cid) = next.get("Ref") {
                cids.insert(cid.as_str().unwrap().to_owned());
            }
        }
        Ok(cids)
    }

    pub fn put_block(&self, cid: &str, block: &TransmissionBlock) -> Result<()> {
        let put_block_url = if cid.starts_with(DAG_PB_CID_PREFIX) {
            format!("{}/block/put?cid-codec=dag-pb", self.address)
        } else {
            format!("{}/block/put", self.address)
        };
        let form_part = multipart::Part::bytes(block.data.to_owned());
        let form = multipart::Form::new().part("data", form_part);
        self.client
            .post(put_block_url)
            .multipart(form)
            .send()?
            .bytes()?;
        Ok(())
    }
}

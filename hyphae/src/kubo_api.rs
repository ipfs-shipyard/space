use anyhow::Result;
use messages::TransmissionBlock;
use reqwest::blocking::multipart;
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::Duration;
use serde::Deserialize;
use tracing::{ warn, debug};

use crate::DAG_PB_CODEC_PREFIX;

pub struct KuboApi {
    address: String,
    client: Client,
}

#[derive(Debug,Deserialize)]
pub struct PutResp {
    #[serde(alias = "Key")]
    pub key: String,
    #[serde(alias = "Size")]
    pub size: u64,
}
#[derive(Debug,Deserialize,Clone)]
pub struct Key {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Name")]
    pub name: String,
}
#[derive(Debug,Deserialize)]
pub struct KeyListResp {
    #[serde(alias = "Keys")]
    pub keys: Vec<Key>
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
                debug!("Found Kubo version {}", resp["Version"]);
                true
            }
            Err(e) => {
                warn!("Could not contact Kubo at this time: {e}");
                false
            }
        }
    }

    pub fn put_block(&self, cid: &str, block: &TransmissionBlock) -> Result<PutResp> {
        let mut put_block_url = format!("{}/block/put?pin=true", self.address);
        if cid.starts_with(DAG_PB_CODEC_PREFIX) {
            put_block_url.push_str("&cid-codec=dag-pb");
        }
        let form_part = multipart::Part::bytes(block.data.to_owned());
        let form = multipart::Form::new().part("data", form_part);
        let resp = self.client
            .post(put_block_url)
            .multipart(form)
            .send()?
            .json::<PutResp>()?;
        Ok(resp)
    }
    pub fn list_keys(&self) -> Result<KeyListResp> {
        let  put_block_url = format!("{}/key/list", self.address);
        let resp = self.client
            .post(put_block_url)
            .send()?
            .json::<KeyListResp>()?;
        Ok(resp)
    }
}

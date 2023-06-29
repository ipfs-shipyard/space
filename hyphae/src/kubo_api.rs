use messages::TransmissionBlock;
use reqwest::blocking::multipart;
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::Duration;
use serde::Deserialize;
use tracing::{warn, debug};
use thiserror::Error;

use crate::DAG_PB_CODEC_PREFIX;

type Result<T> = std::result::Result<T, KuboError>;

pub struct KuboApi {
    address: String,
    client: Client,
}

impl KuboApi {
    pub fn new(address: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
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

    pub fn put_block(&self, cid: &str, block: &TransmissionBlock, pin: bool) -> Result<PutResp> {
        let mut put_block_url = format!("{}/block/put", self.address);
        if pin {
            put_block_url.push_str("?pin=true")
        }
        if cid.starts_with(DAG_PB_CODEC_PREFIX) {
            if pin {
                put_block_url.push('&');
            } else {
                put_block_url.push('?');
            }
            put_block_url.push_str("cid-codec=dag-pb");
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
        let url = format!("{}/key/list", self.address);
        let resp = self.client
            .post(url)
            .send()?
            .json::<KeyListResp>()?;
        Ok(resp)
    }
    pub fn resolve_name(&self, name: &str) -> Result<String> {
        let url = format!("{}/name/resolve?arg={}", self.address, name);
        let resp = self.client
            .post(url)
            .send()?
            .json::<NameResolutionResponse>()?;
        if resp.code == Some(0) {
            Err(KuboError::NoSuchName(name.to_string()))
        } else if let Some(path) = resp.path {
            Ok(path)
        } else if let (Some(code), Some(msg)) = (resp.code, resp.message) {
            Err(KuboError::ServerError(code, msg))
        } else {
            Err(KuboError::Unknown)
        }
    }
    pub fn publish(&self, key_name: &str, target_ipfs_path: &str) -> Result<()> {
        let url = format!("{}/name/publish?arg={}&lifetime=168h&ttl=48h&key={}", self.address, target_ipfs_path, key_name);
        let resp = self.client
            .post(url)
            .send()?
            .json::<GenericResponse>()?;
        if resp.message.is_some() {
            Err(KuboError::ServerError(resp.code.unwrap_or(0), resp.message.unwrap()))
        } else {
            Ok(())
        }
    }
    pub fn get(&self, ipfs_path: &str) -> Result<Vec<u8>> {
        let url = format!("{}/cat?arg={}&progress=false", self.address, ipfs_path);
        let resp = self.client
            .post(url)
            .send()?
            .bytes()?
            .to_vec();
        Ok(resp)
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct GenericResponse {
    #[serde(alias = "Key")]
    pub key: Option<String>,

    #[serde(alias = "Size")]
    pub size: Option<u64>,

    #[serde(alias = "Id")]
    pub id: Option<String>,

    #[serde(alias = "Name")]
    pub name: Option<String>,

    #[serde(alias = "Keys")]
    pub keys: Option<Vec<Key>>,

    #[serde(alias = "Path")]
    path: Option<String>,

    #[serde(alias = "Message")]
    message: Option<String>,

    #[serde(alias = "Code")]
    code: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PutResp {
    #[serde(alias = "Key")]
    pub key: String,
    #[serde(alias = "Size")]
    pub size: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Key {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Name")]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct KeyListResp {
    #[serde(alias = "Keys")]
    pub keys: Vec<Key>,
}

#[derive(Debug, Deserialize)]
pub struct NameResolutionResponse {
    #[serde(alias = "Path")]
    path: Option<String>,
    #[serde(alias = "Message")]
    message: Option<String>,
    #[serde(alias = "Code")]
    code: Option<i64>,
}

#[derive(Debug, Error)]
pub enum KuboError {
    // #[error("JSON response {0} did not contain expected key {1}")]
    // JsonKeyMissing(String, String),
    #[error("Networking problem {0}")]
    ReqwestProblem(#[from] reqwest::Error),
    #[error("Could not resolve the name {0}")]
    NoSuchName(String),
    #[error("The RPC API returned an error: {0}={1}")]
    ServerError(i64, String),
    #[error("Something went wrong.")]
    Unknown,
}

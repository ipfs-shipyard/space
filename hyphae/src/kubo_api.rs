use anyhow::Result;
use reqwest::blocking::Client;
use serde_json::Value;
use tracing::info;

pub struct KuboApi {
    address: String,
    client: Client,
}

impl KuboApi {
    pub fn new(address: &str) -> Self {
        let client = Client::new();
        KuboApi {
            address: format!("http://{}/api/v0", address),
            client,
        }
    }

    pub fn check_alive(&self) -> Result<()> {
        let ping_addr = format!("{}/version", self.address);
        let resp: Value = self.client.post(ping_addr).send()?.json()?;
        info!("Found Kubo version {}", resp["Version"]);
        Ok(())
    }
}

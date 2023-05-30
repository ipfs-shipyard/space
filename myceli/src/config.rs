use anyhow::Result;
use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub listen_address: String,
    pub retry_timeout_duration: u64,
    pub storage_path: String,
    pub mtu: u16,
    pub window_size: u32,
    pub block_size: u32,
    pub chunk_transmit_throttle: Option<u32>,
    pub radio_address: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            // Default listening address
            listen_address: "0.0.0.0:8080".to_string(),
            // Default retry timeout of 120_000 ms = 120 s = 2 minutes
            retry_timeout_duration: 120_000,
            // Default storage dir
            storage_path: "storage".to_string(),
            // Default MTU appropriate for dev radio
            mtu: 512,
            // Default to sending five blocks at a time
            window_size: 5,
            // Default to 3 kilobyte blocks
            block_size: 1024 * 3,
            // Default to no throttling of chunks
            chunk_transmit_throttle: None,
            // Default to no set radio address
            radio_address: None,
        }
    }
}

impl Config {
    pub fn parse(path: Option<String>) -> Result<Self> {
        let mut config = Figment::from(Serialized::defaults(Config::default()));
        if let Some(path) = path {
            config = config.merge(Toml::file(path));
        }
        Ok(config.extract()?)
    }
}

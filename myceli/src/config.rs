use anyhow::Result;
use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::default::Default;
use tracing::info;

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ListenerConfig {
    pub address: String,
    pub retry_timeout_duration: u64,
    pub mtu: u16,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        Self {
            // Default listener address
            address: "127.0.0.1:8080".to_string(),
            // Default retry timeout of 120_000 ms = 120 s = 2 minutes
            retry_timeout_duration: 120_000,
            // Default MTU appropriate for dev radio
            mtu: 60,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub listeners: Vec<ListenerConfig>,
    pub storage_path: String,
    pub name: String,
    pub radio_address: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Default of one listener
            listeners: vec![ListenerConfig::default()],
            // Default storage dir
            storage_path: "storage".to_string(),
            // Default name
            name: "Myceli".to_string(),
            // Default radio address
            radio_address: "127.0.0.1:8081".to_string(),
        }
    }
}

impl Config {
    pub fn parse(path: Option<String>) -> Result<Self> {
        let mut config = Figment::from(Serialized::defaults(Config::default()));
        if let Some(path) = path {
            info!("Myceli running with config values from {path}");
            config = config.merge(Toml::file(path));
        } else {
            info!("Myceli running with default config values");
        }
        Ok(config.extract()?)
    }
}

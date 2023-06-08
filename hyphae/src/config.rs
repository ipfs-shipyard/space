use anyhow::{bail, Result};
use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use transports::MAX_MTU;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub myceli_address: String,
    pub listen_to_myceli_address: String,
    pub kubo_address: String,
    pub sync_interval: u64,
    pub myceli_mtu: u16,
    pub chunk_transmit_throttle: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            myceli_address: "127.0.0.1:8080".to_string(),
            listen_to_myceli_address: "0.0.0.0:8090".to_string(),
            kubo_address: "127.0.0.1:5001".to_string(),
            sync_interval: 10_000,
            myceli_mtu: 512,
            chunk_transmit_throttle: None,
        }
    }
}

impl Config {
    pub fn parse(path: Option<String>) -> Result<Self> {
        let mut config = Figment::from(Serialized::defaults(Config::default()));
        if let Some(path) = path {
            config = config.merge(Toml::file(path));
        }
        let config: Self = config.extract()?;

        if config.myceli_mtu > MAX_MTU {
            bail!("Configured MTU is too large, cannot exceed {MAX_MTU}",);
        }

        Ok(config)
    }
}

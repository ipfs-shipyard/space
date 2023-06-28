use anyhow::{bail, Result};
use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use transports::MAX_MTU;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    // The network address a myceli instance is listening on.
    pub myceli_address: String,
    // The network address to listen on for messages from myceli.
    pub listen_to_myceli_address: String,
    // The network address a kubo instance is listening on.
    pub kubo_address: String,
    // The interval (in milliseconds) to sync with myceli with kubo.
    pub sync_interval: u64,
    // The MTU myceli will use to chunk up messages into UDP packets.
    pub myceli_mtu: u16,
    // The number of milliseconds to wait between sending Message chunks, optional.
    pub chunk_transmit_throttle: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            myceli_address: "0.0.0.0:8001".to_string(),
            listen_to_myceli_address: "0.0.0.0:8100".to_string(),
            kubo_address: "0.0.0.0:5001".to_string(),
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

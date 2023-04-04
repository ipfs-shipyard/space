use anyhow::Result;
use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub myceli_address: String,
    pub kubo_address: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            myceli_address: "127.0.0.1:8080".to_string(),
            kubo_address: "127.0.0.1:5001".to_string(),
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

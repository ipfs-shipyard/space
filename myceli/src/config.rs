use serde_derive::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
struct RawMyceliConfig {
    pub listen_address: Option<String>,
    pub retry_timeout_duration: Option<u64>,
    pub storage_path: Option<String>,
    pub mtu: Option<u16>,
}

pub struct MyceliConfig {
    pub listen_address: String,
    pub retry_timeout_duration: u64,
    pub storage_path: String,
    pub mtu: u16,
}

impl MyceliConfig {
    pub fn parse(path: Option<String>) -> Self {
        let default_config = MyceliConfig::default();
        if let Some(path) = path {
            info!("Myceli running config values from {path}");
            let parsed_cfg = std::fs::read_to_string(path).expect("Failed to read config file");
            let parsed_cfg: RawMyceliConfig =
                toml::from_str(&parsed_cfg).expect("Failed to parse config file");

            return Self {
                listen_address: parsed_cfg
                    .listen_address
                    .unwrap_or(default_config.listen_address),
                retry_timeout_duration: parsed_cfg
                    .retry_timeout_duration
                    .unwrap_or(default_config.retry_timeout_duration),
                storage_path: parsed_cfg
                    .storage_path
                    .unwrap_or(default_config.storage_path),
                mtu: parsed_cfg.mtu.unwrap_or(default_config.mtu),
            };
        } else {
            info!("Myceli running default config values");
        }
        default_config
    }
}

impl std::default::Default for MyceliConfig {
    fn default() -> Self {
        Self {
            // Default listening address
            listen_address: "0.0.0.0:8080".to_string(),
            // Default retry timeout of 120_000 ms = 120 s = 2 minutes
            retry_timeout_duration: 120_000,
            // Default storage dir
            storage_path: "storage".to_string(),
            // Default MTU appropriate for dev radio
            mtu: 60,
        }
    }
}

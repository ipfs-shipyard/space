use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
struct RawMyceliConfig {
    pub listen_address: Option<String>,
    pub retry_timeout_duration: Option<u64>,
    pub storage_path: Option<String>,
}

pub struct MyceliConfig {
    pub listen_address: String,
    pub retry_timeout_duration: u64,
    pub storage_path: String,
}

impl MyceliConfig {
    pub fn parse(path: Option<String>) -> Self {
        let default_config = MyceliConfig::default();
        if let Some(path) = path {
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
            };
        }
        default_config
    }
}

impl std::default::Default for MyceliConfig {
    fn default() -> Self {
        Self {
            listen_address: "127.0.0.1:8080".to_string(),
            retry_timeout_duration: 120,
            storage_path: "storage".to_string(),
        }
    }
}

use anyhow::{bail, Result};
use figment::{
    providers::{Format, Serialized, Toml},
    Figment, Provider,
};
use log::{debug, info, trace};
use serde_derive::{Deserialize, Serialize};
use std::path::PathBuf;

//Duplicated in transport
const MAX_MTU: u16 = 3 * 1024;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    // The network address myceli will listen on for incoming messages.
    pub listen_address: String,
    // The timeout before retrying a dag transfer, measured in milliseconds. This is reset every window.
    pub retry_timeout_duration: u64,
    // Directory path for myceli to use for storage.
    pub storage_path: String,
    // The MTU (in bytes) used to chunk up messages into UDP packets. Maximum value is 3072.
    pub mtu: u16,
    // The number of blocks to send in each window of a DAG transfer.
    pub window_size: u32,
    // The size (in bytes) of the blocks that a file is broken up into when imported.
    pub block_size: Option<u32>,
    // The number of milliseconds to wait between sending chunks of a DAG transfer, optional.
    pub chunk_transmit_throttle: Option<u32>,
    // The network address of the radio that myceli should respond to by default, if not set then
    // myceli will respond to the sending address (or address set in relevant request).
    pub radio_address: Option<String>,
    // A path to a directory which where files that appear should be auto-imported.
    // Absence implies no such directory exists.
    // This value only relevant if using the `watcher` command.
    pub watched_directory: Option<String>,
    //How much storage space should Local Storage use? Measured in kiB. Default is 1 GiB
    pub disk_usage: u64,
    //Minimum amount of time (milliseconds) to elapse between background tasks
    //Note: some background tasks can send a packet on the network depending on circumstance.
    //Default is 10000 (10 seconds).
    //Minimum is 10 (10ms)
    //Maximum is 3600000 (1 hour)
    pub chatter_ms: u32,
    pub shipper_throttle_packet_delay_ms: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            // Default listening address
            listen_address: "0.0.0.0:8001".to_string(),
            // Default retry timeout of 120_000 ms = 120 s = 2 minutes
            retry_timeout_duration: 120_000,
            // Default storage dir
            storage_path: default_storage_path(),
            // Default MTU appropriate for dev radio
            // Maxes out at 1024 * 3 bytes
            mtu: 512,
            // Default to sending five blocks at a time
            window_size: 5,
            // Default to slightly smaller than mtu
            block_size: None,
            // Default to no throttling of chunks
            chunk_transmit_throttle: None,
            // Default to no set radio address
            radio_address: None,
            watched_directory: None,
            disk_usage: 1024 * 1024,
            chatter_ms: 10_000,
            shipper_throttle_packet_delay_ms: 0,
        }
    }
}
fn default_storage_path() -> String {
    dirs::data_local_dir()
        .and_then(|d: PathBuf| {
            d.join("myceli")
                .into_os_string()
                .to_str()
                .map(|s| s.to_owned())
        })
        .unwrap_or_else(|| "storage".to_owned())
}
fn default_config_path() -> Option<String> {
    if let Some(d) = dirs::config_dir() {
        let f = d.join("myceli").join("myceli.toml");
        if f.is_file() {
            return f.into_os_string().into_string().ok();
        }
    }
    None
}
impl Config {
    pub fn parse(path: Option<String>, mtu2block_size: &dyn Fn(u16) -> u16) -> Result<Self> {
        trace!("Config::parse({path:?})");
        let mut config = Figment::from(Serialized::defaults(Config::default()));
        if let Some(path) = path.or(default_config_path()) {
            let toml_values = Toml::file(&path);
            debug!("Config values in file {}: {:?}", &path, toml_values.data());
            config = config.merge(toml_values);
        }
        let mut config: Self = config.extract()?;
        if config.mtu > MAX_MTU {
            bail!("Configured MTU is too large, cannot exceed {MAX_MTU}",);
        }
        if config.block_size.is_none() {
            let sz = mtu2block_size(config.mtu).into();
            info!("Used a mtu {} to deduce block_size {}", config.mtu, sz);
            config.block_size = Some(sz);
        }
        if config.block_size.unwrap() < 128 {
            bail!("block_size too small");
        }
        Ok(config)
    }
}

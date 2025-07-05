use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc};
use once_cell::sync::Lazy;
use serde::Deserialize;

pub const VERSION_PROXY_NAME: &'static str = "0.0.1-unstable";
pub const VERSION_PROTOCOL_NAME: &'static str = "1.20.4";
pub const VERSION_PROTOCOL_CODE: i32 = 765;
pub const BUFFER_SIZE: usize = 4096;

#[derive(PartialEq, PartialOrd, Clone, Debug, Deserialize)]
pub enum LogLevel {
    NONE = 0,
    CONNECTION = 1,
    VERBOSE = 2,
    DEBUG = 3
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConfigSettings {
    pub cache_size: usize,
    pub handshake_timeout: u32,
    pub client_buffer_size: usize,
    pub client_packets_limit: u32,
    pub backend_buffer_size: usize,
    pub ratelimit_window: u32,
    pub ratelimit: u32,
    pub concurrent_limit: u32,
    pub clients_limit: u32,
    pub listen: u16,
    pub log: LogLevel,
    pub log_inspect_buffer_limit: usize
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConfigEndpoint {
    pub hostname: String,
    pub origin: Option<String>,
    pub motd: Option<String>,
    pub message: Option<String>
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub settings: ConfigSettings,
    pub endpoints: Vec<ConfigEndpoint>,
    pub blocklist: Vec<String>
}

impl Config {
    pub fn find_endpoint(&self, addr: String) -> Option<&ConfigEndpoint> {
        self.endpoints.iter().find(|ep| ep.hostname == addr)
    }
}

static CONFIG: Lazy<Arc<Config>> = Lazy::new(|| {
    Arc::new(load_config())
});

fn load_config() -> Config {
    let file = File::open("./config.yaml").expect("Failed to load config.yaml. Does the file exist?");
    let reader = BufReader::new(file);
    let config: Config = serde_yaml::from_reader(reader).expect("Failed to read config.yaml");
    config
}

pub fn get_config() -> Arc<Config> {
    CONFIG.clone()
}

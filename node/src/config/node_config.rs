use crate::config::error::ConfigError;
use crate::config::size_parser::parse_size;
use crate::io::get_space;
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::net::IpAddr;
use std::path::Path;

const MIN_STORAGE_VALUE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct NodeConfig {
    pub cnc_addr: String,
    pub cnc_port: u16,
    pub max_space: String,
    pub ca_certificate: String,
    //internal commons config
    pub addr: String,
    pub port: u16,

    //external certificates config
    pub ssl_certificate: Option<String>,
    pub ssl_private_key: Option<String>,
    pub path: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct NodeConfigInstance {
    pub cnc_addr: String,
    pub cnc_port: u16,
    pub max_space: u64,
    pub ca_certificate: String,
    pub addr: String,
    pub port: u16,
    pub ssl_certificate: Option<String>,
    pub ssl_private_key: Option<String>,
    pub path: String,
}

impl NodeConfig {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return NodeConfig::create_default(path);
            }
            Err(e) => return Err(Box::new(e)),
        };
        let mut contents = String::new();

        file.read_to_string(&mut contents)?;

        Ok(serde_yaml::from_str(&contents)?)
    }

    pub fn create_default(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let default_config = NodeConfig {
            cnc_addr: "127.0.0.1".to_string(),
            cnc_port: 9000,
            ca_certificate: "ca_cert.pem".to_string(),
            max_space: "100mb".to_string(),
            addr: "127.0.0.1".to_string(),
            port: 8080,
            ssl_certificate: None,
            ssl_private_key: None,
            path: "/var/meowith/data/".to_string(),
        };
        let mut new_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        let default_config_yaml = serde_yaml::to_string(&default_config)?;

        new_file.write_all(default_config_yaml.as_bytes())?;
        new_file.sync_all()?;

        Ok(default_config)
    }

    pub async fn validate_config(self) -> Result<NodeConfigInstance, ConfigError> {
        if self.cnc_addr.parse::<IpAddr>().is_err() {
            return Err(ConfigError::InvalidIpAddress);
        }

        if self.cnc_port == 0 {
            return Err(ConfigError::InvalidPort);
        }

        if self.addr.parse::<IpAddr>().is_err() {
            return Err(ConfigError::InvalidIpAddress);
        }

        if self.port == 0 {
            return Err(ConfigError::InvalidPort);
        }

        // The ledger will create the directory if not found.
        let path = Path::new(&self.path);

        let max_space_bytes = parse_size(&self.max_space)?;

        let available_space = get_space(path).await.expect("Disk usage fetch failed");

        if available_space.total < max(MIN_STORAGE_VALUE, max_space_bytes) {
            return Err(ConfigError::InsufficientDiskSpace);
        }

        Ok(NodeConfigInstance {
            cnc_addr: self.cnc_addr,
            cnc_port: self.cnc_port,
            ca_certificate: self.ca_certificate,
            max_space: max_space_bytes,
            addr: self.addr,
            port: self.port,
            ssl_certificate: self.ssl_certificate,
            ssl_private_key: self.ssl_private_key,
            path: if self.path.ends_with('/') {
                self.path
            } else {
                self.path + "/"
            },
        })
    }
}

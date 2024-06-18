use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::net::IpAddr;
use sysinfo::Disks;
use crate::config::error::ConfigError;
use crate::config::size_parser::parse_size;

const MIN_STORAGE_VALUE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct NodeConfig {
    cnc_addr: String,
    cnc_port: u16,
    max_space: String,
    //internal network config
    addr: String,
    port: u16
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct NodeConfigInstance {
    cnc_addr: String,
    cnc_port: u16,
    max_space: u64,
    addr: String,
    port: u16
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
            max_space: "100mb".to_string(),
            addr: "127.0.0.1".to_string(),
            port: 8080,
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

    pub fn validate_config(self) -> Result<NodeConfigInstance, ConfigError> {
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

        let max_space_bytes = parse_size(&self.max_space)?;

        let disks = Disks::new_with_refreshed_list();

        let available_space = disks.iter().map(|disk| disk.available_space()).sum::<u64>();

        if available_space < max_space_bytes + MIN_STORAGE_VALUE {
            return Err(ConfigError::InsufficientDiskSpace);
        }

        Ok(NodeConfigInstance {
            cnc_addr: self.cnc_addr,
            cnc_port: self.cnc_port,
            max_space: max_space_bytes,
            addr: self.addr,
            port: self.port,
        })
    }
}
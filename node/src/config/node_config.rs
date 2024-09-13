use crate::config::error::ConfigError;
use crate::config::size_parser::parse_size;
use crate::io::get_space;
use log::info;
use protocol::mdsftp::MAX_CHUNK_SIZE;
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::net::IpAddr;
use std::path::Path;
use std::str::FromStr;

const MIN_STORAGE_VALUE: u64 = 100 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone)]
pub struct NodeConfig {
    pub cnc_addr: String,
    pub cnc_port: u16,
    pub max_space: String,
    pub ca_certificate: String,
    pub net_fragment_size: u32,

    // internal commons config
    pub external_server_bind_address: String,
    pub external_server_port: u16,
    pub internal_server_bind_address: String,
    pub renewal_token_path: Option<String>,

    // external certificates config
    pub ssl_certificate: Option<String>,
    pub ssl_private_key: Option<String>,
    pub data_save_path: String,

    // Database config
    pub database_nodes: Vec<String>,
    pub db_username: String,
    pub db_password: String,
    pub keyspace: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NodeConfigInstance {
    pub cnc_addr: String,
    pub cnc_port: u16,
    pub max_space: u64,
    pub ca_certificate: String,
    pub external_server_bind_address: String,
    pub external_server_port: u16,
    pub internal_server_bind_address: IpAddr,
    pub ssl_certificate: Option<String>,
    pub ssl_private_key: Option<String>,
    pub renewal_token_path: Option<String>,
    pub data_save_path: String,
    pub net_fragment_size: u32,
    pub database_nodes: Vec<String>,
    pub db_username: String,
    pub db_password: String,
    pub keyspace: String,
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
            cnc_port: 8090,
            ca_certificate: "ca_cert.pem".to_string(),
            max_space: "100mb".to_string(),
            external_server_bind_address: "127.0.0.1".to_string(),
            external_server_port: 8080,
            internal_server_bind_address: "127.0.0.1".to_string(),
            renewal_token_path: None,
            ssl_certificate: None,
            ssl_private_key: None,
            data_save_path: "/var/meowith/data/".to_string(),
            net_fragment_size: 256 * 1024,
            database_nodes: vec!["127.0.0.1".to_string()],
            db_username: "cassandra".to_string(),
            db_password: "cassandra".to_string(),
            keyspace: "meowith".to_string(),
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

        if self.external_server_bind_address.parse::<IpAddr>().is_err() {
            return Err(ConfigError::InvalidIpAddress);
        }

        if self.internal_server_bind_address.parse::<IpAddr>().is_err() {
            return Err(ConfigError::InvalidIpAddress);
        }

        if self.external_server_port == 0 {
            return Err(ConfigError::InvalidPort);
        }

        if self.net_fragment_size > MAX_CHUNK_SIZE as u32 {
            return Err(ConfigError::InvalidFragmentSize);
        }

        // The ledger will create the directory if not found.
        let path = Path::new(&self.data_save_path);

        let max_space_bytes = parse_size(&self.max_space)?;

        let available_space = get_space(path).await.expect("Disk usage fetch failed");
        let requested_space = max(MIN_STORAGE_VALUE, max_space_bytes);
        info!("Available disk space: {available_space:?} Requested: {requested_space}");

        if available_space.total < requested_space {
            return Err(ConfigError::InsufficientDiskSpace);
        }

        Ok(NodeConfigInstance {
            cnc_addr: self.cnc_addr,
            cnc_port: self.cnc_port,
            ca_certificate: self.ca_certificate,
            max_space: max_space_bytes,
            external_server_bind_address: self.external_server_bind_address,
            external_server_port: self.external_server_port,
            internal_server_bind_address: IpAddr::from_str(&self.internal_server_bind_address)
                .unwrap(),
            ssl_certificate: self.ssl_certificate,
            ssl_private_key: self.ssl_private_key,
            renewal_token_path: None,
            data_save_path: if self.data_save_path.ends_with('/') {
                self.data_save_path
            } else {
                self.data_save_path + "/"
            },
            net_fragment_size: self.net_fragment_size,
            database_nodes: self.database_nodes,
            db_username: self.db_username,
            db_password: self.db_password,
            keyspace: self.keyspace,
        })
    }
}

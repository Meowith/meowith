use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};

#[derive(Serialize, Deserialize, Clone)]
pub struct DashboardConfig {
    pub cnc_addr: String,
    pub cnc_port: u16,

    pub ca_certificate: String,

    // external commons config
    pub external_server_bind_address: String,
    pub external_server_port: u16,
    pub self_addr: String,

    pub renewal_token_path: Option<String>,

    // external certificates config
    pub ssl_certificate: Option<String>,
    pub ssl_private_key: Option<String>,

    // Database config
    pub database_nodes: Vec<String>,
    pub db_username: String,
    pub db_password: String,
    pub keyspace: String,
}

impl DashboardConfig {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return DashboardConfig::create_default(path);
            }
            Err(e) => return Err(Box::new(e)),
        };
        let mut contents = String::new();

        file.read_to_string(&mut contents)?;

        Ok(serde_yaml::from_str(&contents)?)
    }

    pub fn create_default(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let default_config = DashboardConfig {
            cnc_addr: "127.0.0.1".to_string(),
            cnc_port: 9000,
            ca_certificate: "ca_cert.pem".to_string(),
            external_server_bind_address: "127.0.0.1".to_string(),
            external_server_port: 8080,
            self_addr: "127.0.0.1".to_string(),
            renewal_token_path: None,
            ssl_certificate: None,
            ssl_private_key: None,
            database_nodes: vec!["127.0.0.1".to_string()],
            db_username: "root".to_string(),
            db_password: "root".to_string(),
            keyspace: "none".to_string(),
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
}

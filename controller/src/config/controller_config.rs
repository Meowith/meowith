use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct ControllerConfig {
    pub discovery_addr: String,
    pub discovery_port: u16,
    pub controller_addr: String,
    pub controller_port: u16,
    pub ssl_certificate: Option<String>,
    pub ssl_private_key: Option<String>,
}

impl ControllerConfig {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return ControllerConfig::create_default(path);
            }
            Err(e) => return Err(Box::new(e)),
        };
        let mut contents = String::new();

        file.read_to_string(&mut contents)?;

        Ok(serde_yaml::from_str(&contents)?)
    }

    pub fn create_default(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let default_config = ControllerConfig {
            discovery_addr: "127.0.0.1".to_string(),
            discovery_port: 8090,
            controller_addr: "127.0.0.1".to_string(),
            controller_port: 8080,
            ssl_certificate: None,
            ssl_private_key: None,
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
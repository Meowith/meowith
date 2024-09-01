use data::dto::config::GeneralConfiguration;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::net::IpAddr;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone)]
pub struct ControllerConfig {
    pub discovery_addr: String,
    pub discovery_port: u16,
    pub controller_addr: String,
    pub controller_port: u16,
    pub setup_addr: String,
    pub setup_port: u16,
    pub ssl_certificate: Option<String>,
    pub ssl_private_key: Option<String>,

    pub ca_certificate: String,
    pub ca_private_key: String,
    pub ca_private_key_password: Option<String>,
    pub autogen_ssl_validity: u32,
    pub internal_ip_addr: IpAddr,

    pub database_nodes: Vec<String>,
    pub db_username: String,
    pub keyspace: String,
    pub db_password: String,

    pub general_configuration: GeneralConfiguration,
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
            setup_addr: "127.0.0.1".to_string(),
            setup_port: 8081,
            ssl_certificate: None,
            ssl_private_key: None,

            ca_certificate: String::from("ca_cert.pem"),
            ca_private_key: String::from("ca_key.pem"),
            ca_private_key_password: Some("my-password".to_string()),
            autogen_ssl_validity: 30,
            internal_ip_addr: IpAddr::from_str("1.2.3.4").unwrap(),

            database_nodes: vec!["127.0.0.1".to_string()],
            db_username: "root".to_string(),
            db_password: "root".to_string(),
            keyspace: "none".to_string(),

            general_configuration: Default::default(),
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

use serde::{Deserialize, Serialize};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GeneralConfiguration {
    pub port_configuration: PortConfiguration,
    pub access_token_configuration: AccessTokenConfiguration,
    pub max_readers: u32,
    pub default_user_quota: u64,
    pub login_methods: Vec<String>,
    pub cat_id_config: Option<CatIdAppConfiguration>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PortConfiguration {
    pub internal_server_port: u16,
    pub mdsftp_server_port: u16,
    pub mgpp_server_port: u16,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CatIdAppConfiguration {
    pub app_id: String,
    pub secret: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AccessTokenConfiguration {
    pub token_validity: u64,
    pub secret: String,
}

impl AccessTokenConfiguration {
    pub fn new() -> Self {
        AccessTokenConfiguration {
            token_validity: 999999999999,
            secret: "".to_string(),
        }
    }
}

impl Default for AccessTokenConfiguration {
    fn default() -> Self {
        AccessTokenConfiguration::new()
    }
}

impl PortConfiguration {
    pub fn new() -> Self {
        PortConfiguration {
            internal_server_port: 21100,
            mdsftp_server_port: 21101,
            mgpp_server_port: 21102,
        }
    }
}

impl Default for PortConfiguration {
    fn default() -> Self {
        PortConfiguration::new()
    }
}

impl GeneralConfiguration {
    pub fn new() -> Self {
        GeneralConfiguration {
            port_configuration: Default::default(),
            access_token_configuration: Default::default(),
            max_readers: 2048u32,
            default_user_quota: 15 * 1024 * 1024 * 1024,
            login_methods: vec!["BASIC".to_string()],
            cat_id_config: None,
        }
    }
}

impl Default for GeneralConfiguration {
    fn default() -> Self {
        GeneralConfiguration::new()
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GeneralConfiguration {
    pub port_configuration: PortConfiguration,
    pub max_readers: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PortConfiguration {
    pub internal_server_port: u16,
    pub mdsftp_server_port: u16,
}

impl PortConfiguration {
    pub fn new() -> Self {
        PortConfiguration {
            internal_server_port: 21100,
            mdsftp_server_port: 21101,
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
            max_readers: 2048u32,
        }
    }
}

impl Default for GeneralConfiguration {
    fn default() -> Self {
        GeneralConfiguration::new()
    }
}

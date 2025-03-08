use controller_lib::config::controller_config::ControllerConfig;
use data::dto::config::{AccessTokenConfiguration, GeneralConfiguration, PortConfiguration};
use lazy_static::lazy_static;

use auth_framework::adapter::r#impl::basic_authenticator::BASIC_TYPE_IDENTIFIER;
use dashboard_lib::dashboard_config::DashboardConfig;
use node_lib::config::node_config::NodeConfigInstance;
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;
use std::string::ToString;

lazy_static! {
    pub static ref TEST_CONTROLLER_CONFIG: ControllerConfig = ControllerConfig {
        discovery_addr: "127.0.0.1".to_string(),
        discovery_port: 2137,
        controller_addr: "127.0.0.1".to_string(),
        controller_port: 2138,
        setup_addr: "127.0.0.1".to_string(),
        setup_port: 2139,
        ssl_certificate: None,
        ssl_private_key: None,
        ca_certificate: "resources/ca_cert.pem".to_string(),
        ca_private_key: "resources/ca_private_key.pem".to_string(),
        ca_private_key_password: Some("admin".to_string()),
        autogen_ssl_validity: 1000,
        internal_ip_addrs: vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))],
        database_nodes: vec!["127.0.0.1:9042".to_string()],
        db_username: "cassandra".to_string(),
        db_password: "cassandra".to_string(),
        keyspace: "meowith_test".to_string(),
        general_configuration: GeneralConfiguration {
            port_configuration: PortConfiguration {
                internal_server_port: 2137,
                mdsftp_server_port: 2139,
                mgpp_server_port: 2140,
            },
            access_token_configuration: AccessTokenConfiguration {
                token_validity: 1000u64,
                secret: "secret".to_string()
            },
            max_readers: 256,
            default_user_quota: 15 * 1024 * 1024 * 1024,
            login_methods: vec![BASIC_TYPE_IDENTIFIER.to_string()],
            cat_id_config: None,
        },
    };
    pub static ref TEST_NODE_1_CONFIG: NodeConfigInstance = NodeConfigInstance {
        cnc_addr: "127.0.0.1".to_string(),
        cnc_port: 2137,
        max_space: 1024 * 1024,
        ca_certificate: "resources/ca_cert.pem".to_string(),
        external_server_bind_address: "127.0.0.2".to_string(),
        external_server_port: 4000,
        internal_server_bind_address: IpAddr::from_str("127.0.0.2").unwrap(),
        broadcast_address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
        cert_addresses: vec![IpAddr::from([127, 0, 0, 2])],
        ssl_certificate: None,
        ssl_private_key: None,
        renewal_token_path: Some("test_data/node1/tkn".to_string()),
        data_save_path: "test_data/node1/data".to_string(),
        net_fragment_size: u16::MAX as u32,
        database_nodes: vec!["127.0.0.1".to_string()],
        db_username: "cassandra".to_string(),
        db_password: "cassandra".to_string(),
        keyspace: "meowith_test".to_string(),
        heart_beat_interval_seconds: 1,
    };
    pub static ref TEST_NODE_2_CONFIG: NodeConfigInstance = NodeConfigInstance {
        cnc_addr: "127.0.0.1".to_string(),
        cnc_port: 2137,
        max_space: 1024 * 1024,
        ca_certificate: "resources/ca_cert.pem".to_string(),
        external_server_bind_address: "127.0.0.3".to_string(),
        external_server_port: 4001,
        internal_server_bind_address: IpAddr::from_str("127.0.0.3").unwrap(),
        broadcast_address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 3)),
        cert_addresses: vec![IpAddr::from([127, 0, 0, 3])],
        ssl_certificate: None,
        ssl_private_key: None,
        renewal_token_path: Some("test_data/node2/tkn".to_string()),
        data_save_path: "test_data/node2/data".to_string(),
        net_fragment_size: u16::MAX as u32,
        database_nodes: vec!["127.0.0.1".to_string()],
        db_username: "cassandra".to_string(),
        db_password: "cassandra".to_string(),
        keyspace: "meowith_test".to_string(),
        heart_beat_interval_seconds: 1,
    };
    pub static ref TEST_DASHBOARD_1_CONFIG: DashboardConfig = DashboardConfig {
        cnc_addr: "127.0.0.1".to_string(),
        cnc_port: 2137,
        ca_certificate: "resources/ca_cert.pem".to_string(),
        external_server_bind_address: "127.0.0.4".to_string(),
        external_server_port: 4002,
        broadcast_address: "127.0.0.4".to_string(),
        renewal_token_path: Some("test_data/wf1/tkn".to_string()),
        cert_addresses: vec!["127.0.0.4".to_string()],
        ssl_certificate: None,
        ssl_private_key: None,
        database_nodes: vec!["127.0.0.1".to_string()],
        db_username: "cassandra".to_string(),
        db_password: "cassandra".to_string(),
        keyspace: "meowith_test".to_string(),
        heart_beat_interval_seconds: 1,
    };
}

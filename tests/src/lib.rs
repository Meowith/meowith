use controller_lib::config::controller_config::ControllerConfig;
use data::dto::config::{AccessTokenConfiguration, GeneralConfiguration, PortConfiguration};
use lazy_static::lazy_static;
lazy_static! {
    static ref TEST_CONTROLLER_CONFIG: ControllerConfig = ControllerConfig {
        discovery_addr: "127.0.0.1".to_string(),
        discovery_port: 2137,
        controller_addr: "127.0.0.1".to_string(),
        controller_port: 2138,
        ssl_certificate: None,
        ssl_private_key: None,
        ca_certificate: "resources/ca_cert.pem".to_string(),
        ca_private_key: "resources/ca_private_key.pem".to_string(),
        ca_private_key_password: Some("admin".to_string()),
        autogen_ssl_validity: 1000000,
        internal_ip_addr: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        database_nodes: vec!["127.0.0.1:9042".to_string()],
        db_username: "cassandra".to_string(),
        db_password: "cassandra".to_string(),
        keyspace: "meowith_test".to_string(),
        general_configuration: GeneralConfiguration {
            port_configuration: PortConfiguration {
                internal_server_port: 2137,
                mdsftp_server_port: 2139,
                catche_server_port: 2140,
            },
            access_token_configuration: AccessTokenConfiguration {
                token_validity: 999999999u64,
                secret: "secret".to_string()
            },
            max_readers: 0,
        },
    };
}

use std::net::{IpAddr, Ipv4Addr};

#[cfg(test)]
mod tests {
    use crate::TEST_CONTROLLER_CONFIG;
    use controller_lib::start_controller;
    use data::database_session::build_raw_session;
    use log::info;
    use migrate::MigrationBuilder;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Duration;
    use tokio::time::sleep;

    async fn initialize_database() {
        let cfg = TEST_CONTROLLER_CONFIG.clone();
        let conn = build_raw_session(
            &cfg.database_nodes,
            &cfg.db_username,
            &cfg.db_password,
            None,
        )
        .await
        .expect("Failed to connect to the test database, is it running?");
        let drop_keyspace = conn
            .prepare(format!("DROP KEYSPACE IF EXISTS {};", &cfg.keyspace))
            .await
            .unwrap();
        conn.execute(&drop_keyspace, ())
            .await
            .expect("Failed to delete previous test data");
        let create_keyspace = conn.prepare(format!("CREATE KEYSPACE {} WITH REPLICATION = {{ 'class' : 'SimpleStrategy', 'replication_factor' : 1 }};;", &cfg.keyspace)).await.unwrap();
        conn.execute(&create_keyspace, ())
            .await
            .expect("Failed to create test data");

        let mut data_path = PathBuf::new();
        data_path.push(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap());
        data_path.push("data");

        let migration = MigrationBuilder::new()
            .keyspace(cfg.keyspace)
            .verbose(true)
            .drop_and_replace(true)
            .build(&conn)
            .await;

        migration.run().await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_controller_boot() {
        info!("Database initialization");
        initialize_database().await;
        info!("Server initialization");
        let stop_handle = start_controller(TEST_CONTROLLER_CONFIG.clone())
            .await
            .expect("BOOT FAILED");
        sleep(Duration::from_secs(1)).await;
        info!("Shutting down...");
        stop_handle.shutdown().await;
        stop_handle.join_handle.await.expect("Join fail");
    }

    // TODO register test
    // TODO file transfer test
}

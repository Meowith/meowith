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
        autogen_ssl_validity: 1000,
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
                token_validity: 1000u64,
                secret: "secret".to_string()
            },
            max_readers: 0,
        },
    };
    static ref TEST_NODE_1_CONFIG: NodeConfigInstance = NodeConfigInstance {
        cnc_addr: "127.0.0.1".to_string(),
        cnc_port: 2137,
        max_space: 1024 * 1024,
        ca_certificate: "resources/ca_cert.pem".to_string(),
        addr: "127.0.0.1".to_string(),
        port: 4000,
        ssl_certificate: None,
        ssl_private_key: None,
        path: "test_data/node1".to_string(),
        net_fragment_size: u16::MAX as u32,
        database_nodes: vec!["127.0.0.1".to_string()],
        db_username: "cassandra".to_string(),
        db_password: "cassandra".to_string(),
        keyspace: "meowith_test".to_string(),
    };
}

use node_lib::config::node_config::NodeConfigInstance;
use std::net::{IpAddr, Ipv4Addr};
use std::string::ToString;

#[cfg(test)]
mod tests {
    use crate::{TEST_CONTROLLER_CONFIG, TEST_NODE_1_CONFIG};
    use controller_lib::public::routes::node_management::RegisterCodeCreateRequest;
    use controller_lib::start_controller;
    use data::database_session::build_raw_session;
    use log::info;
    use logging::initialize_test_logging;
    use migrate::MigrationBuilder;
    use node_lib::start_node;
    use reqwest::ClientBuilder;
    use std::path::{Path, PathBuf};
    use std::time::Duration;
    use std::{env, io};
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

        conn.use_keyspace(&cfg.keyspace, true)
            .await
            .expect("Failed to switch keyspace");

        let migration = MigrationBuilder::new()
            // .project_root(...) Note: await PR
            .verbose(true)
            .build(&conn)
            .await;

        info!("Attempting to run migrations...");

        migration.run().await;
    }

    async fn initialize_directories() -> io::Result<()> {
        let test_data_path = Path::new("test_data");

        if test_data_path.exists() {
            tokio::fs::remove_dir_all(&test_data_path).await?;
        }

        tokio::fs::create_dir_all(test_data_path.join("node1")).await?;
        tokio::fs::create_dir_all(test_data_path.join("node2")).await?;

        Ok(())
    }

    // The tests need to be run in a specific order
    #[tokio::test]
    async fn integration_test_runner() {
        info!("TEST controller boot");
        integration_test_controller_boot().await;

        info!("TEST node register");
        integration_test_register().await;
    }

    async fn integration_test_controller_boot() {
        initialize_test_logging();

        info!("Initializing dirs");
        initialize_directories()
            .await
            .expect("Test dir create failed");

        info!("Database initialization");
        initialize_database().await;

        info!("Server initialization");
        let stop_handle = start_controller(TEST_CONTROLLER_CONFIG.clone())
            .await
            .expect("Controller boot failed");
        sleep(Duration::from_secs(1)).await;

        info!("Shutting down...");
        stop_handle.shutdown().await;
        stop_handle.join_handle.await.expect("Join fail");
    }

    // TODO allow different save path of the renewal token to allow 2 nodes to run simultaneously
    // TODO allow different webserver and mdsftp ports for nodes to allow 2 nodes to run simultaneously
    //      or binding to a specific address ex 127.0.0.2

    async fn integration_test_register() {
        let client = ClientBuilder::new().build().unwrap();

        let controller_stop_handle = start_controller(TEST_CONTROLLER_CONFIG.clone())
            .await
            .expect("Controller boot failed");
        info!("Controller started");

        let code = client
            .post("http://127.0.0.1:2138/api/public/registerCodes/create")
            .send()
            .await
            .expect("")
            .json::<RegisterCodeCreateRequest>()
            .await
            .expect("")
            .code;

        env::set_var("REGISTER_CODE", code);

        let node_1_stop_handle = start_node(TEST_NODE_1_CONFIG.clone())
            .await
            .expect("Failed to register node 1");
        info!("Node started");

        sleep(Duration::from_secs(1)).await;

        node_1_stop_handle.shutdown().await;
        node_1_stop_handle.join_handle.await.expect("Join fail");
        info!("Node shutdown awaited");
        controller_stop_handle.shutdown().await;
        controller_stop_handle.join_handle.await.expect("Join fail");
    }

    // TODO file transfer test
}

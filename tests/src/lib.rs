pub mod file_transfer_test;
pub mod test_configs;
#[macro_use]
pub mod utils;
pub mod durable_file_transfer_test;

#[cfg(test)]
mod tests {
    use crate::durable_file_transfer_test::test_durable_upload;
    use crate::file_transfer_test::test_file_transfer;
    use crate::test_configs::{
        TEST_CONTROLLER_CONFIG, TEST_DASHBOARD_1_CONFIG, TEST_NODE_1_CONFIG, TEST_NODE_2_CONFIG,
    };
    use controller_lib::public::routes::node_management::RegisterCodeCreateRequest;
    use controller_lib::start_controller;
    use dashboard_lib::start_dashboard;
    use data::database_session::build_raw_session;
    use log::{error, info};
    use logging::initialize_test_logging;
    use migrate::MigrationBuilder;
    use node_lib::start_node;
    use reqwest::{Client, ClientBuilder};
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
        data_path.push("src");

        conn.use_keyspace(&cfg.keyspace, true)
            .await
            .expect("Failed to switch keyspace");

        let current_env = env::current_dir().expect("set env failed");
        env::set_current_dir(data_path.to_string_lossy().to_string()).expect("set env failed");
        let migration = MigrationBuilder::new().verbose(true).build(&conn).await;

        info!("Attempting to run migrations...");

        migration.run().await;
        env::set_current_dir(current_env).unwrap()
    }

    async fn initialize_directories() -> io::Result<()> {
        let test_data_path = Path::new("test_data");

        if test_data_path.exists() {
            tokio::fs::remove_dir_all(&test_data_path).await?;
        }

        tokio::fs::create_dir_all(test_data_path.join("node1/data")).await?;
        tokio::fs::create_dir_all(test_data_path.join("node2/data")).await?;
        tokio::fs::create_dir_all(test_data_path.join("wf1")).await?;

        Ok(())
    }

    // The tests need to be run in a specific order
    #[tokio::test]
    #[ntest::timeout(100000)]
    async fn integration_test_runner() {
        let default_panic = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            default_panic(info);
            if let Some(location) = info.location() {
                error!(
                    "panic occurred in file '{}' at line {}",
                    location.file(),
                    location.line(),
                );
            } else {
                error!("panic occurred but can't get location information...");
            }
            std::process::exit(1);
        }));

        big_header!("TEST controller boot");
        integration_test_controller_boot().await;

        big_header!("TEST node register");
        integration_test_register().await;

        let controller_stop_handle = start_controller(TEST_CONTROLLER_CONFIG.clone())
            .await
            .expect("Controller boot failed");
        info!("Controller started");

        let node_1_stop_handle = start_node(TEST_NODE_1_CONFIG.clone())
            .await
            .expect("Failed to register node 1");
        info!("Node started");

        let node_2_stop_handle = start_node(TEST_NODE_2_CONFIG.clone())
            .await
            .expect("Failed to register node 2");
        info!("Node started");

        let dashboard_1_stop_handle = start_dashboard(TEST_DASHBOARD_1_CONFIG.clone())
            .await
            .expect("Failed to register dashboard 1");

        big_header!("TEST file transfer");
        let user_setup = test_file_transfer().await;

        big_header!("TEST durable file transfer");
        test_durable_upload(user_setup).await;

        info!("Shutting down all nodes.");
        node_1_stop_handle.shutdown().await;
        node_1_stop_handle.join_handle.await.expect("Join fail");
        info!("Node 1 shutdown awaited");
        node_2_stop_handle.shutdown().await;
        node_2_stop_handle.join_handle.await.expect("Join fail");
        info!("Node 2 shutdown awaited");
        dashboard_1_stop_handle.shutdown().await;
        dashboard_1_stop_handle
            .join_handle
            .await
            .expect("Join fail");
        info!("Dashboard 1 shutdown awaited");
        controller_stop_handle.shutdown().await;
        controller_stop_handle.join_handle.await.expect("Join fail");
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

    async fn get_code(client: &Client) -> String {
        client
            .post("http://127.0.0.1:2138/api/public/register-codes/create")
            .send()
            .await
            .expect("")
            .json::<RegisterCodeCreateRequest>()
            .await
            .expect("")
            .code
    }

    async fn integration_test_register() {
        let client = ClientBuilder::new().build().unwrap();

        let controller_stop_handle = start_controller(TEST_CONTROLLER_CONFIG.clone())
            .await
            .expect("Controller boot failed");
        info!("Controller started");

        env::set_var("REGISTER_CODE", get_code(&client).await);

        let node_1_stop_handle = start_node(TEST_NODE_1_CONFIG.clone())
            .await
            .expect("Failed to register node 1");
        info!("Node started");

        env::set_var("REGISTER_CODE", get_code(&client).await);

        let node_2_stop_handle = start_node(TEST_NODE_2_CONFIG.clone())
            .await
            .expect("Failed to register node 2");
        info!("Node started");

        env::set_var("REGISTER_CODE", get_code(&client).await);
        let dashboard_1_stop_handle = start_dashboard(TEST_DASHBOARD_1_CONFIG.clone())
            .await
            .expect("Failed to register dashboard 1");

        sleep(Duration::from_secs(1)).await;

        node_1_stop_handle.shutdown().await;
        node_1_stop_handle.join_handle.await.expect("Join fail");
        info!("Node 1 shutdown awaited");
        node_2_stop_handle.shutdown().await;
        node_2_stop_handle.join_handle.await.expect("Join fail");
        info!("Node 2 shutdown awaited");
        dashboard_1_stop_handle.shutdown().await;
        dashboard_1_stop_handle
            .join_handle
            .await
            .expect("Join fail");
        info!("Dashboard 1 shutdown awaited");
        controller_stop_handle.shutdown().await;
        controller_stop_handle.join_handle.await.expect("Join fail");
    }
}

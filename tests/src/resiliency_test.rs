use crate::test_configs::TEST_CONTROLLER_CONFIG;
use crate::utils::Logger;
use chrono::Utc;
use controller_lib::{start_controller, ControllerHandle};
use data::dto::entity::{NodeStatus, NodeStatusResponse};
use http::header::AUTHORIZATION;
use log::{debug, info};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use std::time::Duration;
use tokio::time::sleep;

async fn get_node_status(client: &ClientWithMiddleware, token: &str) -> Vec<NodeStatus> {
    client
        .get("http://127.0.0.1:2138/api/public/node/status")
        .header(AUTHORIZATION, token)
        .send()
        .await
        .expect("")
        .json::<NodeStatusResponse>()
        .await
        .expect("")
        .nodes
}

/// Ensures the nodes pick back up after a controller restart
pub async fn test_controller_reboot_resiliency(
    controller_token: String,
    og_handle: ControllerHandle,
) -> ControllerHandle {
    let reqwest_client = reqwest::Client::builder().build().unwrap();
    let client = ClientBuilder::new(reqwest_client).with(Logger).build();

    og_handle.shutdown().await;
    let controller_stop_handle = start_controller(TEST_CONTROLLER_CONFIG.clone())
        .await
        .expect("Controller boot failed");
    info!("Controller restarted");

    // Wait for the nodes to re-connect
    sleep(Duration::from_secs(5)).await;

    let statuses = get_node_status(&client, &controller_token).await;
    info!("Received statuses {:?}", statuses);

    let now = Utc::now();
    assert!(statuses.iter().all(|x| {
        // Make sure every node's last beat happened sometime after 1970,
        // i.e., the node is recognized as alive by the controller
        debug!(
            "Node {} - {now:?} {} {}",
            x.id,
            x.last_beat,
            now.signed_duration_since(x.last_beat).num_seconds()
        );
        now.signed_duration_since(x.last_beat).num_seconds() < 30
    }));

    controller_stop_handle
}

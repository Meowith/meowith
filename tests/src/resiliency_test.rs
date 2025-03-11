use crate::test_configs::TEST_CONTROLLER_CONFIG;
use crate::utils::Logger;
use chrono::Utc;
use controller_lib::{start_controller, ControllerHandle};
use dashboard_lib::DashboardHandle;
use data::dto::entity::{NodeStatus, NodeStatusResponse};
use http::header::AUTHORIZATION;
use log::{debug, info};
use node_lib::NodeHandle;
use protocol::mgpp::packet::MGPPPacket;
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
    node_1_handle: NodeHandle,
    node_2_handle: NodeHandle,
    dashboard_handle: DashboardHandle,
) -> (ControllerHandle, NodeHandle, NodeHandle, DashboardHandle) {
    let reqwest_client = reqwest::Client::builder().build().unwrap();
    let client = ClientBuilder::new(reqwest_client).with(Logger).build();

    header!("Shutting down the controller");
    og_handle.shutdown().await;

    sleep(Duration::from_secs(5)).await;
    info!("Controller restarted");
    let _ = node_1_handle
        .mgpp_client
        .write_packet(MGPPPacket::InvalidateCache {
            cache_id: 0,
            cache_key: vec![],
        })
        .await;

    let controller_stop_handle = start_controller(TEST_CONTROLLER_CONFIG.clone())
        .await
        .expect("Controller boot failed");

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

    (
        controller_stop_handle,
        node_1_handle,
        node_2_handle,
        dashboard_handle,
    )
}

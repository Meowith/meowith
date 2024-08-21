use logging::initialize_logging;
use node_lib::config::node_config::{NodeConfig, NodeConfigInstance};
use node_lib::{start_node, NodeHandle};
use std::path::Path;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    initialize_logging(Some(Path::new("./log4rs.yaml")));
    let node_config: NodeConfig = NodeConfig::from_file(
        std::env::current_dir()?
            .join("config.yaml")
            .to_str()
            .unwrap(),
    )
    .expect("Failed to init config");

    let config: NodeConfigInstance = node_config
        .validate_config()
        .await
        .expect("Failed to validate config");

    let handle: NodeHandle = start_node(config).await?;

    handle.join_handle.await?;

    Ok(())
}

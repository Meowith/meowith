use crate::config::node_config::NodeConfig;
use crate::init_procedure::register_node;
use logging::initialize_logging;

use commons::autoconfigure::general_conf::fetch_general_config;
use std::path::Path;

mod config;
mod file_transfer;
mod init_procedure;
mod locking;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    initialize_logging(Some(Path::new("./log4rs.yaml")));
    let node_config: NodeConfig = NodeConfig::from_file(
        std::env::current_dir()
            .unwrap()
            .join("config.yaml")
            .to_str()
            .unwrap(),
    )
    .expect("Failed to init config");

    let config = node_config
        .validate_config()
        .expect("Failed to validate config");

    let init_res = register_node(&config).await;
    let mut req_ctx = init_res.0;
    let global_conf = fetch_general_config(&req_ctx).await.unwrap();
    req_ctx.port_configuration = global_conf.port_configuration;
    
    Ok(())
}

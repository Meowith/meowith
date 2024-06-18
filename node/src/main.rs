use crate::config::node_config::NodeConfig;
use crate::init_procedure::send_init_handshake;

mod config;
mod init_procedure;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
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

    send_init_handshake(&config);

    Ok(())
}

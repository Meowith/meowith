use controller_lib::config::controller_config::ControllerConfig;
use controller_lib::{start_controller, ControllerHandle};
use logging::initialize_logging;
use std::path::Path;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    initialize_logging(Some(Path::new("./log4rs.yaml")));
    let config: ControllerConfig = ControllerConfig::from_file(
        std::env::current_dir()?
            .join("config.yaml")
            .to_str()
            .unwrap(),
    )
    .expect("Failed to init config");

    let handle: ControllerHandle = start_controller(config).await?;

    handle.join_handle.await?;

    Ok(())
}

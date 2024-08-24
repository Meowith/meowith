use dashboard_lib::config::DashboardConfig;
use dashboard_lib::{start_dashboard, DashboardHandle};
use logging::initialize_logging;
use std::path::Path;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    initialize_logging(Some(Path::new("./log4rs.yaml")));
    let config: DashboardConfig = DashboardConfig::from_file(
        std::env::current_dir()?
            .join("config.yaml")
            .to_str()
            .unwrap(),
    )
    .expect("Failed to init config");

    let handle: DashboardHandle = start_dashboard(config).await?;

    handle.join_handle.await?;

    Ok(())
}

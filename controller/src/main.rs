use actix_cors::Cors;
use actix_web::dev::Server;
use actix_web::{App, HttpServer};
use futures::future;
use crate::config::controller_config::ControllerConfig;
use crate::ssl::ssl_acceptor_builder::build_ssl_acceptor_builder;

mod ssl;
mod error;
mod discovery;
mod config;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let config: ControllerConfig = ControllerConfig::from_file(
        std::env::current_dir()
            .unwrap()
            .join("config.yaml")
            .to_str()
            .unwrap(),
    )
        .expect("Failed to init config");
    let mut use_ssl = false;

    let discovery_ssl = build_ssl_acceptor_builder(config.clone(), &mut use_ssl);
    let controller_ssl = build_ssl_acceptor_builder(config.clone(), &mut use_ssl);

    let discovery_server = HttpServer::new(|| {
        let cors = Cors::permissive();

        App::new()
            .wrap(cors)
    });

    let controller_server = HttpServer::new(|| {
        let cors = Cors::permissive();

        App::new()
            .wrap(cors)
    });

    let discovery_server_future: Server;
    let controller_server_future: Server;

    if use_ssl {
        discovery_server_future = discovery_server
            .bind_openssl((config.discovery_addr, config.discovery_port), discovery_ssl)?.run();
        controller_server_future = controller_server
            .bind_openssl((config.controller_addr, config.controller_port), controller_ssl)?.run();

    } else {
        discovery_server_future = discovery_server
            .bind((config.discovery_addr, config.discovery_port))?.run();
        controller_server_future = controller_server
            .bind((config.controller_addr, config.controller_port))?.run();
    }

    future::try_join(discovery_server_future, controller_server_future).await?;

    Ok(())
}

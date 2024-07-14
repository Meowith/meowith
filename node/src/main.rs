use crate::config::node_config::{NodeConfig, NodeConfigInstance};
use crate::init_procedure::{initialize_io, register_node};
use logging::initialize_logging;

use crate::io::fragment_ledger::FragmentLedger;
use actix_cors::Cors;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use commons::autoconfigure::general_conf::fetch_general_config;
use commons::context::microservice_request_context::MicroserviceRequestContext;
use commons::ssl_acceptor::build_provided_ssl_acceptor_builder;
use openssl::ssl::SslAcceptorBuilder;
use protocol::file_transfer::server::MDSFTPServer;
use std::path::Path;
use std::sync::Arc;

mod config;
mod file_transfer;
mod init_procedure;
mod io;
mod locking;
mod public;

#[allow(unused)]
pub struct AppState {
    mdsftp_server: MDSFTPServer,
    fragment_ledger: FragmentLedger,
    req_ctx: Arc<MicroserviceRequestContext>,
}

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

    let config: NodeConfigInstance = node_config
        .validate_config()
        .await
        .expect("Failed to validate config");

    let init_res = register_node(&config).await;
    let mut req_ctx = init_res.0;
    let global_conf = fetch_general_config(&req_ctx).await.unwrap();
    req_ctx.port_configuration = global_conf.port_configuration.clone();
    let (internal_cert, internal_key) = (init_res.1.internal_cert, init_res.1.internal_key);
    let req_ctx = Arc::new(req_ctx);

    let (mdsftp_server, fragment_ledger) = initialize_io(
        &internal_cert,
        &internal_key,
        req_ctx.clone(),
        &global_conf,
        &config,
    )
    .await;

    fragment_ledger
        .initialize()
        .await
        .expect("Ledger init failed");

    let mut external_ssl: Option<SslAcceptorBuilder> = None;

    if config.ssl_certificate.is_some() && config.ssl_private_key.is_some() {
        external_ssl = Some(build_provided_ssl_acceptor_builder(
            Path::new(&config.ssl_private_key.clone().unwrap()),
            Path::new(&config.ssl_certificate.clone().unwrap()),
        ));
    }

    let app_data = Data::new(AppState {
        mdsftp_server,
        fragment_ledger,
        req_ctx,
    });

    let external_server = HttpServer::new(move || {
        let cors = Cors::permissive();
        let external_app_data = app_data.clone();

        App::new().app_data(external_app_data).wrap(cors)
    });

    if external_ssl.is_some() {
        external_server
            .bind_openssl((config.addr.clone(), config.port), external_ssl.unwrap())?
            .run()
    } else {
        external_server
            .bind((config.addr.clone(), config.port))?
            .run()
    }
    .await
    .expect("Failed to start external server");

    Ok(())
}

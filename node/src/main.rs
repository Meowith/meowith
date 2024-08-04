use crate::config::node_config::{NodeConfig, NodeConfigInstance};
use crate::init_procedure::{fetch_storage_nodes, initialize_io, register_node};
use logging::initialize_logging;

use crate::caching::catche::connect_catche;
use crate::io::fragment_ledger::FragmentLedger;
use crate::public::service::durable_transfer_session_manager::DurableTransferSessionManager;
use actix_cors::Cors;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use commons::access_token_service::JwtService;
use commons::autoconfigure::general_conf::fetch_general_config;
use commons::context::microservice_request_context::{MicroserviceRequestContext, NodeStorageMap};
use commons::ssl_acceptor::build_provided_ssl_acceptor_builder;
use data::database_session::{build_session, CACHE_SIZE};
use openssl::ssl::SslAcceptorBuilder;
use protocol::mdsftp::server::MDSFTPServer;
use scylla::CachingSession;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

mod caching;
mod config;
mod file_transfer;
mod init_procedure;
mod io;
mod locking;
mod public;

#[allow(unused)]
pub struct AppState {
    session: CachingSession,
    mdsftp_server: MDSFTPServer,
    upload_manager: DurableTransferSessionManager,
    fragment_ledger: FragmentLedger,
    jwt_service: JwtService,
    node_storage_map: NodeStorageMap,
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

    let _ = connect_catche(
        config.cnc_addr.as_str(),
        global_conf.clone(),
        req_ctx.id,
        req_ctx.security_context.root_x509.clone(),
        req_ctx.security_context.access_token.clone(),
    )
    .await;

    let mut external_ssl: Option<SslAcceptorBuilder> = None;

    if config.ssl_certificate.is_some() && config.ssl_private_key.is_some() {
        external_ssl = Some(build_provided_ssl_acceptor_builder(
            Path::new(&config.ssl_private_key.clone().unwrap()),
            Path::new(&config.ssl_certificate.clone().unwrap()),
        ));
    }

    let session = build_session(
        &config.database_nodes,
        &config.db_username,
        &config.db_password,
        CACHE_SIZE,
    )
    .await
    .expect("Unable to connect to database");

    let app_data = Data::new(AppState {
        session,
        mdsftp_server,
        upload_manager: DurableTransferSessionManager::new(),
        fragment_ledger,
        jwt_service: JwtService::new(&global_conf.access_token_configuration)
            .expect("JWT Service creation failed"),
        node_storage_map: Arc::new(RwLock::new(
            fetch_storage_nodes(&req_ctx)
                .await
                .expect("Failed to fetch storage nodes")
                .peers,
        )),
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

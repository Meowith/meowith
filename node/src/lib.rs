use crate::config::node_config::NodeConfigInstance;
use crate::init_procedure::{fetch_storage_nodes, initialize_io, register_node};

use crate::caching::catche::connect_catche;
use crate::io::fragment_ledger::FragmentLedger;
use crate::public::service::durable_transfer_session_manager::DurableTransferSessionManager;
use actix_cors::Cors;
use actix_web::dev::ServerHandle;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use commons::access_token_service::JwtService;
use commons::autoconfigure::general_conf::fetch_general_config;
use commons::context::microservice_request_context::{MicroserviceRequestContext, NodeStorageMap};
use commons::ssl_acceptor::build_provided_ssl_acceptor_builder;
use data::database_session::{build_session, CACHE_SIZE};
use openssl::ssl::SslAcceptorBuilder;
use protocol::catche::catche_client::CatcheClient;
use protocol::mdsftp::server::MDSFTPServer;
use scylla::CachingSession;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

pub mod caching;
pub mod config;
pub mod file_transfer;
pub mod init_procedure;
pub mod io;
pub mod locking;
pub mod public;

pub struct NodeHandle {
    external_handle: ServerHandle,
    mdsftp_server: MDSFTPServer,
    catche_client: CatcheClient,
    pub join_handle: JoinHandle<()>,
}

impl NodeHandle {
    pub async fn shutdown(&self) {
        self.external_handle.stop(true).await;
        self.catche_client.shutdown().await;
        self.mdsftp_server.shutdown().await;
    }
}

pub struct AppState {
    session: CachingSession,
    mdsftp_server: MDSFTPServer,
    upload_manager: DurableTransferSessionManager,
    fragment_ledger: FragmentLedger,
    jwt_service: JwtService,
    node_storage_map: NodeStorageMap,
    req_ctx: Arc<MicroserviceRequestContext>,
}

pub async fn start_node(config: NodeConfigInstance) -> std::io::Result<NodeHandle> {
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

    let catche_client = connect_catche(
        config.cnc_addr.as_str(),
        global_conf.clone(),
        req_ctx.id,
        req_ctx.security_context.root_x509.clone(),
        req_ctx.security_context.access_token.clone(),
    )
    .await
    .expect("Catche conncetion failed");

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
        Some(&config.keyspace),
        CACHE_SIZE,
    )
    .await
    .expect("Unable to connect to database");

    let mdsftp_server_clone = mdsftp_server.clone();

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
    app_data.upload_manager.init_session(app_data.clone()).await;

    let external_server = HttpServer::new(move || {
        let cors = Cors::permissive();
        let external_app_data = app_data.clone();

        App::new().app_data(external_app_data).wrap(cors)
    });

    let external_server = if external_ssl.is_some() {
        external_server
            .bind_openssl((config.addr.clone(), config.port), external_ssl.unwrap())?
            .run()
    } else {
        external_server
            .bind((config.addr.clone(), config.port))?
            .run()
    };
    let external_handle = external_server.handle();

    let join_handle = tokio::task::spawn(async {
        if let Err(err) = external_server.await {
            log::error!("Node server mdsftp_error {err:?}");
        }
    });

    Ok(NodeHandle {
        external_handle,
        catche_client,
        mdsftp_server: mdsftp_server_clone,
        join_handle,
    })
}

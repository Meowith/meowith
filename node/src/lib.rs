extern crate core;

use crate::config::node_config::NodeConfigInstance;
use crate::init_procedure::{initialize_heart, initialize_io, register_node};
use std::collections::HashMap;

use crate::caching::clear_caches;
use crate::io::fragment_ledger::FragmentLedger;
use crate::public::middleware::user_middleware::UserAuthenticate;
use crate::public::routes::entity_action::{
    create_directory, delete_directory, delete_file, rename_directory, rename_file,
};
use crate::public::routes::entity_list::{
    get_bucket_info, list_bucket_directories, list_bucket_files, list_directory, stat_entity,
};
use crate::public::routes::file_transfer::{
    download, resume_durable_upload, start_upload_durable, upload_durable, upload_oneshot,
};
use crate::public::service::durable_transfer_session_manager::DurableTransferSessionManager;
use actix_cors::Cors;
use actix_web::dev::ServerHandle;
use actix_web::web::Data;
use actix_web::{web, App, HttpServer};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use commons::access_token_service::AccessTokenJwtService;
use commons::autoconfigure::general_conf::fetch_general_config;
use commons::context::microservice_request_context::{MicroserviceRequestContext, NodeStorageMap};
use commons::error::std_response::NodeClientError;
use commons::pause_handle::ApplicationPauseHandle;
use commons::ssl_acceptor::build_provided_ssl_acceptor_builder;
use data::database_session::{build_session, CACHE_SIZE};
use log::trace;
use mgpp::connect_mgpp;
use openssl::ssl::SslAcceptorBuilder;
use peer::peer_utils::fetch_peer_storage_info;
use protocol::mdsftp::server::MDSFTPServer;
use protocol::mgpp::client::MGPPClient;
use scylla::CachingSession;
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, RwLock};
use tokio::task::{AbortHandle, JoinHandle};
use uuid::Uuid;

pub mod caching;
pub mod config;
pub mod file_transfer;
pub mod init_procedure;
pub mod io;
pub mod locking;
pub mod mgpp;
pub mod peer;
pub mod public;

pub struct NodeHandle {
    external_handle: ServerHandle,
    mdsftp_server: MDSFTPServer,
    pub mgpp_client: MGPPClient,
    heart_handle: AbortHandle,
    req_ctx: Arc<MicroserviceRequestContext>,
    pub join_handle: JoinHandle<()>,
    fragment_ledger: FragmentLedger,
}

impl NodeHandle {
    pub async fn shutdown(&self, forceful: bool) {
        self.external_handle.stop(!forceful).await;
        self.heart_handle.abort();
        let _ = self.mgpp_client.shutdown().await;
        self.mdsftp_server.shutdown().await;
        self.req_ctx.shutdown().await;
        self.fragment_ledger.shutdown().await;
    }
}

pub struct AppState {
    session: CachingSession,
    mdsftp_server: MDSFTPServer,
    upload_manager: DurableTransferSessionManager,
    fragment_ledger: FragmentLedger,
    jwt_service: AccessTokenJwtService,
    node_storage_map: NodeStorageMap,
    req_ctx: Arc<MicroserviceRequestContext>,
    pause_handle: Arc<Mutex<Option<ServerHandle>>>,
    last_peer_refresh: Arc<Mutex<DateTime<Utc>>>,
}

impl AppState {
    pub async fn pause(&self) {
        self.pause_handle
            .lock()
            .await
            .as_ref()
            .unwrap()
            .pause()
            .await;
        self.fragment_ledger.pause();
        clear_caches().await;
    }

    pub async fn resume(&self) {
        clear_caches().await;
        self.pause_handle
            .lock()
            .await
            .as_ref()
            .unwrap()
            .resume()
            .await;
        self.fragment_ledger.resume();
    }

    /// Returns true if a refresh has been performed
    pub async fn safe_refresh_peer_data(&self) -> Result<bool, NodeClientError> {
        let mut last = self.last_peer_refresh.lock().await;
        let now = Utc::now();
        if now.signed_duration_since(*last).num_seconds()
            >= self.req_ctx.heart_beat_interval_seconds as i64
        {
            *last = now;
            drop(last); // Avoid a deadlock
            self.refresh_peer_data().await.map(|_| true)
        } else {
            Ok(false)
        }
    }

    pub async fn refresh_peer_data(&self) -> Result<(), NodeClientError> {
        let peers = fetch_peer_storage_info(&self.req_ctx)
            .await
            .map_err(|_| NodeClientError::InternalError)?
            .peers;
        let mut map = self.node_storage_map.write().await;
        let mut last = self.last_peer_refresh.lock().await;
        for peer in peers {
            map.insert(peer.0, peer.1.storage);
        }
        *last = Utc::now();
        trace!("New peer data {:?}", map);
        Ok(())
    }
}

struct NodePauseHandle {
    state: Data<AppState>,
}

#[async_trait]
impl ApplicationPauseHandle for NodePauseHandle {
    async fn pause(&self) {
        self.state.pause().await;
    }

    async fn resume(&self) {
        self.state.resume().await;
    }
}

pub async fn start_node(config: NodeConfigInstance) -> std::io::Result<NodeHandle> {
    let init_res = register_node(&config).await;
    let mut req_ctx = init_res.0;
    let global_conf = fetch_general_config(&req_ctx).await.unwrap();
    let fs_limit_configuration = web::Data::new(global_conf.fs_limits.clone());
    req_ctx.port_configuration = global_conf.port_configuration.clone();
    let (internal_cert, internal_key) = (init_res.1.internal_cert, init_res.1.internal_key);
    let req_ctx = Arc::new(req_ctx);
    let req_ctx_handle = req_ctx.clone();

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

    let peers = fetch_peer_storage_info(&req_ctx)
        .await
        .expect("Failed to fetch storage nodes")
        .peers;
    let storage_map: HashMap<Uuid, u64> = peers
        .clone()
        .into_iter()
        .map(|elem| (elem.0, elem.1.storage))
        .collect();
    {
        let mut nodes = req_ctx.node_addr.write().await;
        for peer in peers {
            nodes.insert(peer.0, peer.1.addr.to_string());
        }
    }

    let node_storage_map = Arc::new(RwLock::new(storage_map));
    req_ctx
        .update_storage(fragment_ledger.update_req().await)
        .await
        .expect("Update storage failed");

    let mgpp_client = connect_mgpp(
        config.cnc_addr.as_str(),
        global_conf.clone(),
        req_ctx.id,
        req_ctx.security_context.root_x509.clone(),
        req_ctx.security_context.access_token.clone(),
        (node_storage_map.clone(), req_ctx.clone()),
    )
    .await
    .expect("MGPP connection failed");
    let heart_req_ctx = req_ctx.clone();
    let heart_ledger = fragment_ledger.clone();
    let pause_handle = Arc::new(Mutex::new(None));
    let app_data = Data::new(AppState {
        session,
        mdsftp_server,
        upload_manager: DurableTransferSessionManager::new(),
        fragment_ledger: fragment_ledger.clone(),
        jwt_service: AccessTokenJwtService::new(&global_conf.access_token_configuration)
            .expect("JWT Service creation failed"),
        node_storage_map,
        req_ctx,
        pause_handle: pause_handle.clone(),
        last_peer_refresh: Arc::new(Default::default()),
    });
    app_data.upload_manager.init_session(app_data.clone()).await;

    let node_pause_handle: Arc<Box<dyn ApplicationPauseHandle>> =
        Arc::new(Box::new(NodePauseHandle {
            state: app_data.clone(),
        }));
    mgpp_client
        .set_up_auto_reconnect(node_pause_handle.clone())
        .await;

    let external_server = HttpServer::new(move || {
        let cors = Cors::permissive();
        let external_app_data = app_data.clone();
        let fs_limit_configuration = fs_limit_configuration.clone();

        let file_scope = web::scope("/api/file")
            .service(upload_oneshot)
            .service(upload_durable)
            .service(start_upload_durable)
            .service(resume_durable_upload)
            .service(download)
            .service(rename_file)
            .service(delete_file)
            .wrap(UserAuthenticate);

        let directory_scope = web::scope("/api/directory")
            .service(create_directory)
            .service(delete_directory)
            .service(rename_directory)
            .service(list_directory)
            .wrap(UserAuthenticate);

        let bucket_scope = web::scope("/api/bucket")
            .service(list_bucket_files)
            .service(list_bucket_directories)
            .service(stat_entity)
            .service(get_bucket_info)
            .wrap(UserAuthenticate);

        App::new()
            .app_data(external_app_data)
            .app_data(fs_limit_configuration)
            .wrap(cors)
            .service(file_scope)
            .service(directory_scope)
            .service(bucket_scope)
    });

    let external_server = if external_ssl.is_some() {
        external_server
            .bind_openssl(
                (
                    config.external_server_bind_address.clone(),
                    config.external_server_port,
                ),
                external_ssl.unwrap(),
            )?
            .run()
    } else {
        external_server
            .bind((
                config.external_server_bind_address.clone(),
                config.external_server_port,
            ))?
            .run()
    };
    pause_handle.lock().await.replace(external_server.handle());
    let external_handle = external_server.handle();

    let join_handle = tokio::task::spawn(async move {
        if let Err(err) = external_server.await {
            log::error!("Node server mdsftp_error {err:?}");
        }
        log::info!("Node server stopped.");
    });

    let heart_handle = initialize_heart(heart_req_ctx, heart_ledger);

    Ok(NodeHandle {
        fragment_ledger,
        external_handle,
        mgpp_client,
        mdsftp_server: mdsftp_server_clone,
        req_ctx: req_ctx_handle,
        join_handle,
        heart_handle,
    })
}

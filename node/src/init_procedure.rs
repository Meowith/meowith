use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use reqwest::Certificate;
use tokio::sync::Mutex;
use uuid::Uuid;

use commons::autoconfigure::auth_conf::{register_procedure, RegistrationResult};
use commons::context::microservice_request_context::{MicroserviceRequestContext, SecurityContext};
use commons::context::request_context::RequestContext;
use data::dto::config::GeneralConfiguration;
use data::dto::controller::StorageResponse;
use data::model::microservice_node_model::MicroserviceType;
use protocol::mdsftp::authenticator::ConnectionAuthContext;
use protocol::mdsftp::pool::{MDSFTPPoolConfigHolder, PacketHandlerRef};
use protocol::mdsftp::server::MDSFTPServer;

use crate::config::node_config::NodeConfigInstance;
use crate::file_transfer::connection_authenticator::MeowithMDSFTPConnectionAuthenticator;
use crate::file_transfer::packet_handler::MeowithMDSFTPPacketHandler;
use crate::io::fragment_ledger::{FragmentLedger, LockTable};
use crate::locking::file_lock_table::FileLockTable;

pub async fn register_node(
    config: &NodeConfigInstance,
) -> (MicroserviceRequestContext, RegistrationResult) {
    let ca_cert = X509::from_pem(
        fs::read(&config.ca_certificate)
            .expect("Unable to read ca cert file")
            .as_slice(),
    )
    .expect("Invalid ca cert format");

    let security_ctx = SecurityContext {
        access_token: "".to_string(),
        renewal_token: "".to_string(),
        root_x509: ca_cert.clone(),
        root_certificate: Certificate::from_pem(ca_cert.to_pem().unwrap().as_slice())
            .expect("Invalid certificate file"),
    };

    let mut ctx = MicroserviceRequestContext::new(
        config.cnc_addr.clone(),
        HashMap::new(),
        security_ctx,
        MicroserviceType::StorageNode,
        Default::default(),
        Uuid::new_v4(),
    );

    let reg_res = register_procedure(&mut ctx).await;

    (ctx, reg_res)
}

pub async fn initialize_io(
    cert: &X509,
    key: &PKey<Private>,
    req_ctx: Arc<MicroserviceRequestContext>,
    global_config: &GeneralConfiguration,
    config: &NodeConfigInstance,
) -> (MDSFTPServer, FragmentLedger) {
    let authenticator = MeowithMDSFTPConnectionAuthenticator {
        req_ctx: req_ctx.clone(),
    };

    let connection_auth_context = Arc::new(ConnectionAuthContext {
        root_certificate: req_ctx.security_context.root_x509.clone(),
        authenticator: Some(Box::new(authenticator)),
        port: req_ctx.port_configuration.mdsftp_server_port,
        own_id: req_ctx.id,
    });

    let lock_table: LockTable = FileLockTable::new(global_config.max_readers);
    let ledger = FragmentLedger::new(config.path.clone(), config.max_space, lock_table);
    let handler: PacketHandlerRef = Arc::new(Mutex::new(Box::new(
        MeowithMDSFTPPacketHandler::new(ledger.clone(), config.net_fragment_size),
    )));

    let cfg = MDSFTPPoolConfigHolder {
        fragment_size: config.net_fragment_size,
        buffer_size: 16,
    };

    let mut server = MDSFTPServer::new(
        connection_auth_context.clone(),
        req_ctx.node_addr.clone(),
        handler,
        cfg,
    )
    .await;
    server
        .start(cert, key)
        .await
        .expect("Failed to stat the MDSFTP server");

    (server, ledger)
}

pub async fn fetch_storage_nodes(
    req_ctx: &MicroserviceRequestContext,
) -> reqwest::Result<StorageResponse> {
    req_ctx
        .client()
        .await
        .get(req_ctx.controller("/api/internal/health/storage"))
        .send()
        .await?
        .json::<StorageResponse>()
        .await
}

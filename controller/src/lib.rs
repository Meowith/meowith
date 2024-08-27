use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::catche::catche::{start_server, ControllerAuthenticator};
use crate::config::controller_config::ControllerConfig;
use crate::discovery::routes::{
    authenticate_node, config_fetch, register_node, security_csr, validate_peer,
};
use crate::health::routes::{
    fetch_free_storage, microservice_heart_beat, update_storage_node_properties,
};
use crate::ioutils::read_file;
use crate::middleware::node_internal::NodeVerify;
use crate::public::routes::node_management::create_register_code;
use actix_cors::Cors;
use actix_web::dev::{Server, ServerHandle};
use actix_web::web::Data;
use actix_web::{web, App, HttpServer};
use commons::autoconfigure::ssl_conf::{generate_csr, generate_private_key, sign_csr, SigningData};
use commons::context::controller_request_context::ControllerRequestContext;
use commons::ssl_acceptor::{
    build_autogen_ssl_acceptor_builder, build_provided_ssl_acceptor_builder,
};
use data::access::microservice_node_access::get_microservices;
use data::database_session::{build_session, CACHE_SIZE};
use data::model::microservice_node_model::MicroserviceNode;
use futures::future;
use log::{debug, error, info};
use openssl::pkey::{PKey, Private};
use openssl::ssl::SslAcceptorBuilder;
use openssl::x509::X509;
use protocol::catche::catche_server::CatcheServer;
use reqwest::Certificate;
use scylla::CachingSession;
use tokio::task;
use tokio::task::JoinHandle;

pub mod catche;
pub mod config;
pub mod discovery;
pub mod error;
pub mod health;
pub mod ioutils;
pub mod middleware;
pub mod public;
pub mod token_service;

pub struct AppState {
    session: CachingSession,
    config: ControllerConfig,
    pub ca_cert: X509,
    pub ca_private_key: PKey<Private>,
    req_ctx: ControllerRequestContext,
    catche_server: CatcheServer,
}

pub struct ControllerHandle {
    internode_server_handle: ServerHandle,
    public_server_handle: ServerHandle,
    catche_server: CatcheServer,
    pub join_handle: JoinHandle<()>,
}

impl ControllerHandle {
    pub async fn shutdown(&self) {
        self.public_server_handle.stop(true).await;
        self.internode_server_handle.stop(true).await;
        self.catche_server.shutdown().await;
    }
}

pub async fn start_controller(config: ControllerConfig) -> std::io::Result<ControllerHandle> {
    let clonfig = config.clone();
    let mut use_ssl = false;
    let mut controller_ssl: Option<SslAcceptorBuilder> = None;

    if clonfig.ssl_certificate.is_some() && clonfig.ssl_private_key.is_some() {
        controller_ssl = Some(build_provided_ssl_acceptor_builder(
            Path::new(&clonfig.ssl_private_key.clone().unwrap()),
            Path::new(&clonfig.ssl_certificate.clone().unwrap()),
        ));
        use_ssl = true;
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

    let ca_cert = X509::from_pem(
        read_file(&config.ca_certificate)
            .expect("Unable to read ca cert file")
            .as_slice(),
    )
    .expect("Invalid ca cert format");
    let ca_private_key_file =
        read_file(&config.ca_private_key).expect("Unable to read ca private key file");

    let ca_private_key = if let Some(pass) = &config.ca_private_key_password {
        PKey::private_key_from_pem_passphrase(ca_private_key_file.as_slice(), pass.as_bytes())
            .expect("Invalid ca key format")
    } else {
        PKey::private_key_from_pem(ca_private_key_file.as_slice()).expect("Invalid ca key format")
    };

    let microservices_iter = get_microservices(&session)
        .await
        .expect("Unable to fetch service nodes");
    let mut node_addr_map = HashMap::new();
    let mut node_token_map = HashMap::new();
    let mut token_node_map = HashMap::new();

    let mut nodes: Vec<MicroserviceNode> = Vec::new();
    for node in microservices_iter {
        nodes.push(node.unwrap());
    }

    for node in &nodes {
        node_addr_map.insert(node.id, node.address.clone().to_string());
        if node.access_token.is_some() {
            node_token_map.insert(node.id, node.access_token.clone().unwrap());
            token_node_map.insert(node.access_token.clone().unwrap(), node.clone());
        }
    }

    let req_ctx = ControllerRequestContext::new(
        node_addr_map,
        node_token_map,
        token_node_map,
        nodes,
        Certificate::from_pem(ca_cert.to_pem().unwrap().as_slice())
            .expect("Invalid certificate file"),
    );
    let internal_certs = create_internal_certs((ca_cert.clone(), ca_private_key.clone()), &clonfig);

    let catche = start_server(
        config
            .clone()
            .general_configuration
            .port_configuration
            .catche_server_port,
        ca_cert.clone(),
        ControllerAuthenticator {
            req_ctx: Arc::new(req_ctx.clone()),
        },
        internal_certs.clone(),
    )
    .await;
    let app_data = Data::new(AppState {
        session,
        config: config.clone(),
        ca_cert: ca_cert.clone(),
        ca_private_key: ca_private_key.clone(),
        req_ctx: req_ctx.clone(),
        catche_server: catche.clone(),
    });

    let init_app_data = app_data.clone();

    let internode_server = HttpServer::new(move || {
        let internode_app_data = app_data.clone();
        let cors = Cors::permissive();
        let internal_scope = web::scope("/api/internal")
            .wrap(NodeVerify {})
            .service(validate_peer)
            .service(config_fetch)
            .service(security_csr);

        let health_scope = web::scope("/api/internal/health")
            .wrap(NodeVerify {})
            .service(update_storage_node_properties)
            .service(fetch_free_storage)
            .service(microservice_heart_beat);

        let init_scope = web::scope("/api/internal/initialize")
            .service(authenticate_node)
            .service(register_node);

        App::new()
            .app_data(internode_app_data)
            .wrap(cors)
            .service(init_scope)
            .service(health_scope)
            .service(internal_scope)
    });

    let controller_server = HttpServer::new(move || {
        let controller_app_data = init_app_data.clone();
        let cors = Cors::permissive();
        let register_codes = web::scope("/api/public/registerCodes").service(create_register_code);

        App::new()
            .wrap(cors)
            .app_data(controller_app_data)
            .service(register_codes)
    });

    let internode_server_future: Server;
    let controller_server_future: Server;

    let internode_ssl = build_autogen_ssl_acceptor_builder(internal_certs.0, internal_certs.1);

    internode_server_future = internode_server
        .bind_openssl(
            (clonfig.discovery_addr.clone(), clonfig.discovery_port),
            internode_ssl,
        )?
        .run();

    info!(
        "Starting the internode server on {}:{} using SSL",
        clonfig.discovery_addr, clonfig.discovery_port
    );

    if use_ssl && controller_ssl.is_some() {
        controller_server_future = controller_server
            .bind_openssl(
                (clonfig.controller_addr.clone(), clonfig.controller_port),
                controller_ssl.unwrap(),
            )?
            .run();
        info!(
            "Starting the public server on {}:{} using SSL",
            clonfig.controller_addr, clonfig.controller_port
        );
    } else {
        controller_server_future = controller_server
            .bind((clonfig.controller_addr.clone(), clonfig.controller_port))?
            .run();
        info!(
            "Starting the public server on {}:{}",
            clonfig.controller_addr, clonfig.controller_port
        );
    }

    let internode_server_handle = internode_server_future.handle();
    let public_server_handle = controller_server_future.handle();

    let join_handle = task::spawn(async move {
        if let Err(err) = future::try_join(internode_server_future, controller_server_future).await
        {
            error!("Server mdsftp_error {err:?}");
        }
    });

    Ok(ControllerHandle {
        internode_server_handle,
        public_server_handle,
        catche_server: catche,
        join_handle,
    })
}

fn create_internal_certs(
    state: (X509, PKey<Private>),
    config: &ControllerConfig,
) -> (X509, PKey<Private>) {
    let key = generate_private_key().expect("Key gen failed");
    let request = generate_csr(&key).expect("CSR gen failed");

    let cert = sign_csr(
        &request,
        &state.0,
        &state.1,
        &SigningData {
            ip_addr: config.internal_ip_addr,
            validity_days: 3560, // Note: consider auto-renewal
        },
    )
    .expect("CRS sign failed");

    debug!("Created internode SSL certificates");

    (cert, key)
}

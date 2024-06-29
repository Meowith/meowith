use std::collections::HashMap;
use std::path::Path;

use actix_cors::Cors;
use actix_rt::Runtime;
use actix_web::dev::Server;
use actix_web::web::Data;
use actix_web::{web, App, HttpServer};
use futures::future;
use openssl::pkey::{PKey, Private};
use openssl::ssl::SslAcceptorBuilder;
use openssl::x509::X509;
use reqwest::Certificate;
use scylla::CachingSession;

use data::access::microservice_node_access::get_microservices;
use data::database_session::build_session;
use network::autoconfigure::ssl_conf::{generate_csr, generate_private_key, sign_csr, SigningData};
use network::context::ControllerRequestContext;
use network::ssl_acceptor::{
    build_autogen_ssl_acceptor_builder, build_provided_ssl_acceptor_builder,
};

use crate::config::controller_config::ControllerConfig;
use crate::discovery::routes::{register_node, security_csr};
use crate::ioutils::read_file;

mod config;
mod discovery;
mod error;
mod ioutils;
mod middleware;
mod token_service;

pub struct AppState {
    session: CachingSession,
    config: ControllerConfig,
    ca_cert: X509,
    ca_private_key: PKey<Private>,
    req_ctx: ControllerRequestContext,
}

fn create_internal_certs(
    state: &Data<AppState>,
    config: &ControllerConfig,
) -> (X509, PKey<Private>) {
    let key = generate_private_key().expect("Key gen failed");
    let request = generate_csr(&key).expect("CSR gen failed");

    let cert = sign_csr(
        &request,
        &state.ca_cert.clone(),
        &state.ca_private_key.clone(),
        &SigningData {
            ip_addr: config.internal_ip_addr,
            validity_days: 3560, // Note: consider auto-renewal
        },
    )
    .expect("CRS sign failed");

    (cert, key)
}

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

    let runtime = Runtime::new().unwrap();

    let session = runtime
        .block_on(build_session(
            &config.database_nodes,
            &config.db_username,
            &config.db_password,
            1,
        ))
        .expect("Unable to connect to database");

    let ca_cert = X509::from_pem(
        read_file(&config.ca_certificate)
            .expect("Unable to read ca cert file")
            .as_slice(),
    )
    .expect("Invalid ca cert format");
    let ca_private_key = PKey::private_key_from_pem(
        read_file(&config.ca_private_key)
            .expect("Unable to read ca private key file")
            .as_slice(),
    )
    .expect("Invalid private key format");

    let microservices_iter = get_microservices(&session)
        .await
        .expect("Unable to fetch service nodes");
    let mut node_addr_map = HashMap::new();
    let mut node_token_map = HashMap::new();
    let mut token_node_map = HashMap::new();

    for node in microservices_iter {
        let node = node.expect("Invalid node config");
        node_addr_map.insert(node.id, node.address.clone().to_string());
        node_token_map.insert(node.id, node.token.clone());
        token_node_map.insert(node.token.clone(), node.clone());
    }

    let req_ctx = ControllerRequestContext {
        node_addr: node_addr_map,
        node_token: node_token_map,
        token_node: token_node_map,
        root_certificate: Certificate::from_pem(ca_cert.to_pem().unwrap().as_slice())
            .expect("Invalid certificate file"),
    };

    let app_data = web::Data::new(AppState {
        session,
        config: config.clone(),
        ca_cert: ca_cert.clone(),
        ca_private_key: ca_private_key.clone(),
        req_ctx: req_ctx.clone(),
    });

    let init_app_data = app_data.clone();
    let internode_server = HttpServer::new(move || {
        let internode_app_data = app_data.clone();
        let cors = Cors::permissive();
        let internal_scope = web::scope("/api/internal")
            .service(register_node)
            .service(security_csr);

        App::new()
            .app_data(internode_app_data)
            .service(internal_scope)
            .wrap(cors)
    });

    let controller_server = HttpServer::new(|| {
        let cors = Cors::permissive();

        App::new().wrap(cors)
    });

    let internode_server_future: Server;
    let controller_server_future: Server;

    let internal_certs = create_internal_certs(&init_app_data, &clonfig);
    let internode_ssl = build_autogen_ssl_acceptor_builder(internal_certs.0, internal_certs.1);

    internode_server_future = internode_server
        .bind_openssl(
            (clonfig.discovery_addr, clonfig.discovery_port),
            internode_ssl,
        )?
        .run();

    if use_ssl && controller_ssl.is_some() {
        controller_server_future = controller_server
            .bind_openssl(
                (clonfig.controller_addr, clonfig.controller_port),
                controller_ssl.unwrap(),
            )?
            .run();
    } else {
        controller_server_future = controller_server
            .bind((clonfig.controller_addr, clonfig.controller_port))?
            .run();
    }

    future::try_join(internode_server_future, controller_server_future).await?;

    Ok(())
}

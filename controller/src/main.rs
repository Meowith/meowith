use std::collections::HashMap;
use crate::config::controller_config::ControllerConfig;
use crate::discovery::routes::{register_node, security_csr};
use crate::ssl::ssl_acceptor_builder::build_ssl_acceptor_builder;
use actix_cors::Cors;
use actix_rt::Runtime;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use data::database_session::build_session;
use futures::future;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use scylla::CachingSession;
use std::error::Error;
use std::fs::File;
use std::io::{Read};
use std::sync::Arc;
use reqwest::Certificate;
use data::access::microservice_node_access::get_microservices;
use network::context::ControllerRequestContext;
use crate::database_session::{get_session, set_session};

mod config;
mod discovery;
mod error;
mod ssl;
mod middleware;
mod database_session;

pub struct AppState {
    session: CachingSession,
    config: ControllerConfig,
    ca_cert: X509,
    ca_private_key: PKey<Private>,
    req_ctx: ControllerRequestContext
}

fn read_file(path: &String) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) => return Err(Box::new(e)),
    };
    let mut buffer: Vec<u8> = Vec::new();

    file.read_to_end(&mut buffer)?;

    Ok(buffer)
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
    let discovery_ssl = build_ssl_acceptor_builder(config.clone(), &mut use_ssl);
    let controller_ssl = build_ssl_acceptor_builder(config.clone(), &mut use_ssl);
    let runtime = Runtime::new().unwrap();

    let session = runtime
        .block_on(build_session(
            &config.database_nodes,
            &config.db_username,
            &config.db_password,
            1
        ))
        .expect("Unable to connect to database");

    let ca_cert =
        X509::from_pem(read_file(&config.ca_certificate).expect("Unable to read ca cert file").as_slice())
            .expect("Invalid ca cert format");
    let ca_private_key = PKey::private_key_from_pem(
        read_file(&config.ca_private_key).expect("Unable to read ca private key file").as_slice(),
    ).expect("Invalid private key format");

    let microservices_iter = get_microservices(&session).await.expect("Unable to fetch service nodes");
    let mut node_addr_map = HashMap::new();
    let mut node_token_map = HashMap::new();
    let mut token_node_map = HashMap::new();

    for node in microservices_iter {
        let node = node.expect("Invalid node config");
        node_addr_map.insert(node.id.clone(), node.address.clone().to_string());
        node_token_map.insert(node.id.clone(), node.token.clone());
        token_node_map.insert(node.token.clone(), node.clone());
    }

    let req_ctx = ControllerRequestContext {
        node_addr: node_addr_map,
        node_token: node_token_map,
        token_node: token_node_map,
        root_certificate: Certificate::from_pem(ca_cert.to_pem().unwrap().as_slice()).expect("Invalid certificate file"),
    };

    set_session(session);

    let discovery_server = HttpServer::new(move || {
        let cors = Cors::permissive();
        let internal_scope = web::scope("/api/internal")
            .service(register_node)
            .service(security_csr);

        App::new()
            .app_data(web::Data::new(AppState {
                session: get_session(),
                config: config.clone(),
                ca_cert: ca_cert.clone(),
                ca_private_key: ca_private_key.clone(),
                req_ctx: req_ctx.clone(),
            }))
            .service(internal_scope)
            .wrap(cors)
    });

    let controller_server = HttpServer::new(|| {
        let cors = Cors::permissive();

        App::new().wrap(cors)
    });

    let discovery_server_future: Server;
    let controller_server_future: Server;

    if use_ssl {
        discovery_server_future = discovery_server
            .bind_openssl(
                (clonfig.discovery_addr, clonfig.discovery_port),
                discovery_ssl,
            )?
            .run();
        controller_server_future = controller_server
            .bind_openssl(
                (clonfig.controller_addr, clonfig.controller_port),
                controller_ssl,
            )?
            .run();
    } else {
        discovery_server_future = discovery_server
            .bind((clonfig.discovery_addr, clonfig.discovery_port))?
            .run();
        controller_server_future = controller_server
            .bind((clonfig.controller_addr, clonfig.controller_port))?
            .run();
    }

    future::try_join(discovery_server_future, controller_server_future).await?;

    Ok(())
}

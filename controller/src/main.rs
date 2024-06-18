use actix_cors::Cors;
use actix_rt::Runtime;
use actix_web::dev::Server;
use actix_web::{App, HttpServer, web};
use futures::future;
use scylla::CachingSession;
use data::database_session::build_session;
use crate::config::controller_config::ControllerConfig;
use crate::discovery::routes::register_node;
use crate::ssl::ssl_acceptor_builder::build_ssl_acceptor_builder;

mod ssl;
mod error;
mod discovery;
mod config;

pub struct AppState {
    session: CachingSession,
    config: ControllerConfig,
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

    let discovery_server = HttpServer::new(move || {
        let runtime = Runtime::new().unwrap();
        let cors = Cors::permissive();
        let internal_scope = web::scope("/api/internal")
            .service(register_node);
        let session = runtime.block_on(build_session(
            &config.database_nodes,
            &config.db_username,
            &config.db_password,
            10
        )).expect("Unable to connect to database");

        App::new()
            .app_data(web::Data::new(AppState {
                session,
                config: config.clone()
            }))
            .service(internal_scope)
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
            .bind_openssl((clonfig.discovery_addr, clonfig.discovery_port), discovery_ssl)?.run();
        controller_server_future = controller_server
            .bind_openssl((clonfig.controller_addr, clonfig.controller_port), controller_ssl)?.run();

    } else {
        discovery_server_future = discovery_server
            .bind((clonfig.discovery_addr, clonfig.discovery_port))?.run();
        controller_server_future = controller_server
            .bind((clonfig.controller_addr, clonfig.controller_port))?.run();
    }

    future::try_join(discovery_server_future, controller_server_future).await?;

    Ok(())
}

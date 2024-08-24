use crate::auth::method_container::{init_authentication_methods, AuthenticationMethodList};
use crate::auth::token::AuthenticationJwtService;
use crate::auth::user_middleware::UserMiddlewareRequestTransform;
use crate::caching::catche::connect_catche;
use crate::config::DashboardConfig;
use crate::init_procedure::register_node;
use crate::public::auth::auth_routes::{login, register};
use crate::public::routes::application::{create_application, delete_application};
use crate::public::routes::bucket::create_bucket;
use crate::public::routes::token::issue_app_token;
use actix_cors::Cors;
use actix_web::dev::ServerHandle;
use actix_web::web::Data;
use actix_web::{web, App, HttpServer};
use auth::r#impl::basic_authenticator::BASIC_TYPE_IDENTIFIER;
use commons::access_token_service::AccessTokenJwtService;
use commons::autoconfigure::general_conf::fetch_general_config;
use commons::context::microservice_request_context::MicroserviceRequestContext;
use commons::ssl_acceptor::build_provided_ssl_acceptor_builder;
use data::database_session::{build_session, CACHE_SIZE};
use log::error;
use openssl::ssl::SslAcceptorBuilder;
use protocol::catche::catche_client::CatcheClient;
use scylla::CachingSession;
use std::path::Path;
use std::sync::Arc;
use tokio::task::JoinHandle;
// TODO middleware for dashboard
// TODO middleware for controller admin

pub mod auth;
pub mod caching;
pub mod config;
pub mod init_procedure;
pub mod public;

pub struct DashboardHandle {
    external_handle: ServerHandle,
    catche_client: CatcheClient,
    req_ctx: Arc<MicroserviceRequestContext>,
    pub join_handle: JoinHandle<()>,
}

impl DashboardHandle {
    pub async fn shutdown(&self) {
        self.external_handle.stop(true).await;
        self.catche_client.shutdown().await;
        self.req_ctx.shutdown().await;
    }
}

#[allow(unused)]
pub struct AppState {
    session: CachingSession,
    jwt_service: AccessTokenJwtService,
    authentication_jwt_service: AuthenticationJwtService,
    catche_client: CatcheClient,
    authentication: AuthenticationMethodList,
    req_ctx: Arc<MicroserviceRequestContext>,
}

pub async fn start_dashboard(config: DashboardConfig) -> std::io::Result<DashboardHandle> {
    let init_res = register_node(&config).await;
    let mut req_ctx = init_res.0;
    let global_conf = fetch_general_config(&req_ctx).await.unwrap();
    req_ctx.port_configuration = global_conf.port_configuration.clone();
    let _ = (init_res.1.internal_cert, init_res.1.internal_key);
    let req_ctx = Arc::new(req_ctx);
    let req_ctx_handle = req_ctx.clone();

    let catche_client = connect_catche(
        config.cnc_addr.as_str(),
        global_conf.clone(),
        req_ctx.id,
        req_ctx.security_context.root_x509.clone(),
        req_ctx.security_context.access_token.clone(),
    )
    .await
    .expect("Catche connection failed");

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

    let auth = init_authentication_methods(&config).expect("Invalid authentication methods");
    let has_basic = auth.contains_key(BASIC_TYPE_IDENTIFIER);

    let app_data = Data::new(AppState {
        session,
        jwt_service: AccessTokenJwtService::new(&global_conf.access_token_configuration)
            .expect("JWT Service creation failed"),
        authentication_jwt_service: AuthenticationJwtService::new(
            &global_conf.access_token_configuration,
        )
        .expect("Authentication JWT Service creation failed"),
        catche_client: catche_client.clone(),
        authentication: Arc::new(auth),
        req_ctx,
    });

    let external_server = HttpServer::new(move || {
        let cors = Cors::permissive();
        let external_app_data = app_data.clone();
        let mut auth_scope = web::scope("/auth").service(login);
        let app_scope = web::scope("/app")
            .wrap(UserMiddlewareRequestTransform)
            .service(create_application)
            .service(delete_application)
            .service(web::scope("/token").service(issue_app_token));
        let bucket_scope = web::scope("/bucket")
            .wrap(UserMiddlewareRequestTransform)
            .service(create_bucket);

        if has_basic {
            auth_scope = auth_scope.service(register);
        }

        App::new().app_data(external_app_data).wrap(cors).service(
            web::scope("/api")
                .service(auth_scope)
                .service(app_scope)
                .service(bucket_scope),
        )
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
    let external_handle = external_server.handle();

    let join_handle = tokio::task::spawn(async {
        if let Err(err) = external_server.await {
            error!("Node server mdsftp_error {err:?}");
        }
    });

    Ok(DashboardHandle {
        external_handle,
        catche_client,
        req_ctx: req_ctx_handle,
        join_handle,
    })
}

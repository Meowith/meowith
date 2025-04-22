use crate::auth::user_middleware::UserMiddlewareRequestTransform;
use crate::caching::clear_caches;
use crate::dashboard_config::DashboardConfig;
use crate::init_procedure::{initializer_heart, register_node};
use crate::public::auth::auth_routes::{get_methods, login, own_user_info, register};
use crate::public::routes::application::{
    add_member, buckets, create_application, delete_application, delete_member, edit_application,
    list_members, list_owned,
};
use crate::public::routes::bucket::{
    create_bucket, delete_bucket_handler, edit_bucket, get_sessions,
};
use crate::public::routes::role::{
    create_role, delete_role, get_roles, modify_role, update_roles_for_member,
};
use crate::public::routes::token::{delete_token, issue_app_token, list_tokens};
use crate::public::routes::user::{user_by_id, user_by_name};
use actix_cors::Cors;
use actix_web::dev::ServerHandle;
use actix_web::web::Data;
use actix_web::{web, App, HttpServer};
use async_trait::async_trait;
use auth_framework::adapter::method_container::{
    init_authentication_methods, AuthenticationMethodList,
};
use auth_framework::adapter::r#impl::basic_authenticator::BASIC_TYPE_IDENTIFIER;
use auth_framework::adapter::token::AuthenticationJwtService;
use commons::access_token_service::AccessTokenJwtService;
use commons::autoconfigure::general_conf::fetch_general_config;
use commons::context::microservice_request_context::MicroserviceRequestContext;
use commons::pause_handle::ApplicationPauseHandle;
use commons::ssl_acceptor::build_provided_ssl_acceptor_builder;
use data::database_session::{build_session, CACHE_SIZE};
use data::dto::config::GeneralConfiguration;
use log::error;
use mgpp::connect_mgpp;
use openssl::ssl::SslAcceptorBuilder;
use protocol::mgpp::client::MGPPClient;
use scylla::client::caching_session::CachingSession;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::{AbortHandle, JoinHandle};

pub mod auth;
pub mod caching;
pub mod dashboard_config;
pub mod init_procedure;
pub mod mgpp;
pub mod public;

pub struct DashboardHandle {
    external_handle: ServerHandle,
    mgpp_client: MGPPClient,
    heart_handle: AbortHandle,
    req_ctx: Arc<MicroserviceRequestContext>,
    pub join_handle: JoinHandle<()>,
}

impl DashboardHandle {
    pub async fn shutdown(&self) {
        self.heart_handle.abort();
        self.external_handle.stop(true).await;
        self.req_ctx.shutdown().await;
        let _ = self.mgpp_client.shutdown().await;
    }
}

pub struct AppState {
    session: CachingSession,
    jwt_service: AccessTokenJwtService,
    authentication_jwt_service: AuthenticationJwtService,
    mgpp_client: MGPPClient,
    authentication: AuthenticationMethodList,
    #[allow(unused)]
    req_ctx: Arc<MicroserviceRequestContext>,
    global_config: GeneralConfiguration,
}

struct DashboardPauseHandle {
    pause_handle: Arc<Mutex<Option<ServerHandle>>>,
}

#[async_trait]
impl ApplicationPauseHandle for DashboardPauseHandle {
    async fn pause(&self) {
        self.pause_handle
            .lock()
            .await
            .as_mut()
            .unwrap()
            .pause()
            .await;
        // Clear after pausing to avoid stale data being loaded into the cache after pause.
        clear_caches().await;
    }

    async fn resume(&self) {
        // Clear before serving requests to ensure no request gets stale data.
        clear_caches().await;
        self.pause_handle
            .lock()
            .await
            .as_mut()
            .unwrap()
            .resume()
            .await;
    }
}

pub async fn start_dashboard(config: DashboardConfig) -> std::io::Result<DashboardHandle> {
    let init_res = register_node(&config).await;
    let mut req_ctx = init_res.0;
    let global_conf = fetch_general_config(&req_ctx).await.unwrap();
    req_ctx.port_configuration = global_conf.port_configuration.clone();
    let _ = (init_res.1.internal_cert, init_res.1.internal_key);
    let req_ctx = Arc::new(req_ctx);
    let req_ctx_handle = req_ctx.clone();

    let mgpp_client = connect_mgpp(
        config.cnc_addr.as_str(),
        global_conf.clone(),
        req_ctx.id,
        req_ctx.security_context.root_x509.clone(),
        req_ctx.security_context.access_token.clone(),
    )
    .await
    .expect("MGPP connection failed");

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

    let auth = init_authentication_methods(global_conf.login_methods.clone(), global_conf.clone())
        .expect("Invalid authentication methods");
    let has_basic = auth.contains_key(BASIC_TYPE_IDENTIFIER);

    let pause_handle = Arc::new(Mutex::new(None));
    let app_data = Data::new(AppState {
        session,
        jwt_service: AccessTokenJwtService::new(&global_conf.access_token_configuration)
            .expect("JWT Service creation failed"),
        authentication_jwt_service: AuthenticationJwtService::new(
            &global_conf.access_token_configuration,
        )
        .expect("Authentication JWT Service creation failed"),
        mgpp_client: mgpp_client.clone(),
        authentication: Arc::new(auth),
        req_ctx,
        global_config: global_conf,
    });

    let node_pause_handle: Arc<Box<dyn ApplicationPauseHandle>> =
        Arc::new(Box::new(DashboardPauseHandle {
            pause_handle: pause_handle.clone(),
        }));
    mgpp_client
        .set_up_auto_reconnect(node_pause_handle.clone())
        .await;

    let external_server = HttpServer::new(move || {
        let cors = Cors::permissive();
        let external_app_data = app_data.clone();
        let mut auth_scope = web::scope("/auth").service(login).service(get_methods);
        let app_scope = web::scope("/app")
            .wrap(UserMiddlewareRequestTransform)
            .service(create_application)
            .service(delete_application)
            .service(edit_application)
            .service(list_owned)
            .service(buckets)
            .service(add_member)
            .service(delete_member)
            .service(list_members)
            .service(
                web::scope("/token")
                    .service(issue_app_token)
                    .service(delete_token)
                    .service(list_tokens),
            );

        let bucket_scope = web::scope("/bucket")
            .wrap(UserMiddlewareRequestTransform)
            .service(delete_bucket_handler)
            .service(edit_bucket)
            .service(get_sessions)
            .service(create_bucket);

        let role_scope = web::scope("/role")
            .wrap(UserMiddlewareRequestTransform)
            .service(get_roles)
            .service(create_role)
            .service(delete_role)
            .service(modify_role)
            .service(update_roles_for_member);

        let user_scope = web::scope("/public/user")
            .service(own_user_info)
            .service(user_by_id)
            .service(user_by_name)
            .wrap(UserMiddlewareRequestTransform);

        if has_basic {
            auth_scope = auth_scope.service(register);
        }

        App::new().app_data(external_app_data).wrap(cors).service(
            web::scope("/api")
                .service(auth_scope)
                .service(app_scope)
                .service(bucket_scope)
                .service(role_scope)
                .service(user_scope),
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
    pause_handle.lock().await.replace(external_server.handle());

    let join_handle = tokio::task::spawn(async {
        if let Err(err) = external_server.await {
            error!("Node server mdsftp_error {err:?}");
        }
    });

    let heart_handle = initializer_heart(req_ctx_handle.clone());

    Ok(DashboardHandle {
        external_handle,
        mgpp_client,
        req_ctx: req_ctx_handle,
        join_handle,
        heart_handle,
    })
}

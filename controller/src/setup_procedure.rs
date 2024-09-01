use crate::config::controller_config::ControllerConfig;
use crate::setup::auth_routes::{login, register, EmptyResponse};
use actix_cors::Cors;
use actix_web::dev::Server;
use actix_web::web::Data;
use actix_web::{get, web, App, HttpServer};
use auth_framework::adapter::method_container::AuthMethodMap;
use auth_framework::adapter::r#impl::basic_authenticator::BASIC_TYPE_IDENTIFIER;
use log::info;
use openssl::ssl::SslAcceptorBuilder;
use scylla::CachingSession;
use std::error::Error;
use tokio::select;
use tokio_util::sync::CancellationToken;

pub struct SetupAppState {
    pub(crate) session: CachingSession,
    pub(crate) auth: AuthMethodMap,
    pub(crate) shutdown: CancellationToken,
}

#[get("/state")]
pub async fn setup_status() -> web::Json<EmptyResponse> {
    web::Json(EmptyResponse)
}

pub async fn setup_controller(
    clonfig: ControllerConfig,
    auth: AuthMethodMap,
    ssl: Option<SslAcceptorBuilder>,
    session: CachingSession,
) -> Result<(), Box<dyn Error>> {
    let shutdown_token = CancellationToken::new();

    let has_basic = auth.contains_key(BASIC_TYPE_IDENTIFIER);

    let app_state = Data::new(SetupAppState {
        session,
        auth,
        shutdown: shutdown_token.clone(),
    });

    let setup_server = HttpServer::new(move || {
        let cors = Cors::permissive();
        let mut auth_scope = web::scope("/api/auth").service(login).service(setup_status);

        if has_basic {
            auth_scope = auth_scope.service(register);
        }

        App::new()
            .app_data(app_state.clone())
            .wrap(cors)
            .service(auth_scope)
    });
    let setup_server_future: Server;

    if let Some(ssl) = ssl {
        setup_server_future = setup_server
            .bind_openssl((clonfig.setup_addr.clone(), clonfig.setup_port), ssl)?
            .run();
        info!(
            "Starting the setup server on {}:{} using SSL",
            clonfig.setup_addr, clonfig.setup_port
        );
    } else {
        setup_server_future = setup_server
            .bind((clonfig.setup_addr.clone(), clonfig.setup_port))?
            .run();
        info!(
            "Starting the setup server on {}:{}",
            clonfig.setup_addr, clonfig.setup_port
        );
    }
    let server_handle = setup_server_future.handle();
    let shutdown_future = shutdown_token.cancelled();

    select! {
        _ = setup_server_future => {},
        _ = shutdown_future => {
            server_handle.stop(true).await
        }
    }

    Ok(())
}

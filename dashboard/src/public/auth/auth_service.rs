use crate::public::auth::auth_routes::{AuthResponse, RegisterRequest};
use crate::AppState;
use actix_web::HttpRequest;
use auth_framework::token::DashboardClaims;
use bcrypt::{hash, DEFAULT_COST};
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::access::user_access::insert_user;
use data::model::permission_model::GlobalRole;
use data::model::user_model::User;
use uuid::Uuid;

pub async fn do_login(
    req: HttpRequest,
    method: String,
    state: &AppState,
) -> NodeClientResponse<AuthResponse> {
    let facade = state
        .authentication
        .get(&method)
        .ok_or(NodeClientError::BadRequest)?;

    let credentials = facade.convert(&req).map_err(|_| NodeClientError::BadAuth)?;

    let claims = facade
        .authenticate(credentials, &state.session)
        .await
        .map_err(|_| NodeClientError::BadAuth)?;

    let token = state.authentication_jwt_service.generate_token(claims)?;

    Ok(AuthResponse { token })
}

pub async fn do_register(
    req: RegisterRequest,
    state: &AppState,
) -> NodeClientResponse<AuthResponse> {
    let user = User {
        id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        name: req.username,
        auth_identifier: hash(
            req.password + auth_framework::adapter::r#impl::basic_authenticator::PEPPER,
            DEFAULT_COST,
        )?,
        global_role: GlobalRole::User.into(),
        created: Default::default(),
        last_modified: Default::default(),
    };

    insert_user(&user, &state.session).await?;

    let token = state
        .authentication_jwt_service
        .generate_token(DashboardClaims {
            id: user.id,
            sid: user.session_id,
        })?;

    Ok(AuthResponse { token })
}

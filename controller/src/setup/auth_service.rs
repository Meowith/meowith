use actix_web::HttpRequest;
use bcrypt::{hash, DEFAULT_COST};
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::access::user_access::insert_user;
use data::model::permission_model::GlobalRole;
use data::model::user_model::User;
use uuid::Uuid;
use auth_framework::adapter::r#impl::basic_authenticator::BASIC_TYPE_IDENTIFIER;
use auth_framework::credentials::AuthenticationCredentials;
use crate::setup::auth_routes::{RegisterRequest};
use crate::setup_procedure::SetupAppState;

#[derive(Debug)]
pub struct SetupCredentials {
    credentials: Box<dyn AuthenticationCredentials>
}

impl AuthenticationCredentials for SetupCredentials {
    fn get_authentication_identifier(&self) -> String {
        self.credentials.get_authentication_identifier()
    }

    fn get_username(&self) -> Option<String> {
        self.credentials.get_username()
    }

    fn is_setup(&self) -> bool {
        true
    }
}

pub async fn do_login(
    req: HttpRequest,
    method: String,
    state: &SetupAppState,
) -> NodeClientResponse<()> {
    if method.to_uppercase() == BASIC_TYPE_IDENTIFIER {
        return Err(NodeClientError::BadRequest);
    }

    let facade = state
        .auth
        .get(&method)
        .ok_or(NodeClientError::BadRequest)?;

    let credentials = facade.convert(&req).map_err(|_| NodeClientError::BadAuth)?;

    facade
        .authenticate(Box::new(SetupCredentials {
            credentials,
        }), &state.session)
        .await
        .map_err(|_| NodeClientError::BadAuth)?;

    state.shutdown.cancel();

    Ok(())
}

pub async fn do_register(
    req: RegisterRequest,
    state: &SetupAppState,
) -> NodeClientResponse<()> {

    // First user is admin, amazing
    let user = User {
        id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        name: req.username,
        auth_identifier: hash(
            req.password + auth_framework::adapter::r#impl::basic_authenticator::PEPPER,
            DEFAULT_COST,
        )?,
        global_role: GlobalRole::Admin.into(),
        created: Default::default(),
        last_modified: Default::default(),
    };

    insert_user(&user, &state.session).await?;

    state.shutdown.cancel();
    Ok(())
}

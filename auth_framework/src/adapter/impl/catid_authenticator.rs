use crate::credentials::AuthenticationCredentials;
use crate::error::{AuthCredentialsError, AuthenticateError};
use crate::token::DashboardClaims;
use crate::{AuthFacade, Authentication};
use actix_web::HttpRequest;
use async_trait::async_trait;
use chrono::Utc;
use data::access::user_access::{get_user_from_auth, insert_user, update_user};
use data::dto::config::{CatIdAppConfiguration, GeneralConfiguration};
use data::model::permission_model::GlobalRole;
use data::model::user_model::User;
use reqwest::Client;
use scylla::client::caching_session::CachingSession;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CATID_TYPE_IDENTIFIER: &str = "CATID";
pub const CATID_API_URL: &str = "https://idapi.michal.cat/api/app/user";
pub const CATID_TOKEN_URL: &str = "https://idapi.michal.cat/api/app/token";

#[derive(Debug)]
pub struct CatIdAuthenticator {
    client: Client,
    config: CatIdAppConfiguration,
    general_config: GeneralConfiguration,
}

impl CatIdAuthenticator {
    pub fn new(config: GeneralConfiguration) -> Self {
        Self {
            client: Client::builder().build().unwrap(),
            config: config.cat_id_config.clone().unwrap(),
            general_config: config,
        }
    }
}

#[derive(Debug)]
pub struct CatIdCredentials {
    code: String,
}

impl AuthenticationCredentials for CatIdCredentials {
    fn get_authentication_identifier(&self) -> String {
        self.code.clone()
    }

    fn get_username(&self) -> Option<String> {
        None
    }
}

#[derive(Serialize)]
pub struct AccessTokenRequest {
    app_id: String,
    code: String,
    secret: String,
}

#[derive(Deserialize)]
pub struct AccessTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
pub struct CatIdUser {
    id: String,
    name: String,
}

#[async_trait]
impl Authentication for CatIdAuthenticator {
    async fn authenticate(
        &self,
        credentials: Box<dyn AuthenticationCredentials>,
        session: &CachingSession,
    ) -> Result<DashboardClaims, AuthenticateError> {
        let credentials = credentials;

        let token = self
            .client
            .post(CATID_TOKEN_URL)
            .body(
                serde_json::to_string(&AccessTokenRequest {
                    app_id: self.config.app_id.clone(),
                    code: credentials.get_authentication_identifier(),
                    secret: self.config.secret.clone(),
                })
                .unwrap(),
            )
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|_| AuthenticateError::InvalidCredentials)?
            .json::<AccessTokenResponse>()
            .await
            .map_err(|_| AuthenticateError::InvalidCredentials)?
            .access_token;

        let cat_user = self
            .client
            .get(CATID_API_URL)
            .header("Authorization", token)
            .send()
            .await
            .map_err(|_| AuthenticateError::InternalError)?
            .json::<CatIdUser>()
            .await
            .map_err(|_| AuthenticateError::InvalidCredentials)?;

        let user = get_user_from_auth(cat_user.id.clone(), session).await;

        if let Ok(user) = user {
            if cat_user.name != user.name {
                update_user(user.id, cat_user.name, session)
                    .await
                    .map_err(|_| AuthenticateError::InternalError)?;
            }

            Ok(DashboardClaims {
                id: user.id,
                sid: user.session_id,
            })
        } else {
            let now = Utc::now();
            let user = User {
                id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
                name: cat_user.name,
                auth_identifier: cat_user.id,
                quota: self.general_config.default_user_quota as i64,
                global_role: if credentials.is_setup() {
                    GlobalRole::Admin.into()
                } else {
                    GlobalRole::User.into()
                },
                created: now,
                last_modified: now,
            };

            insert_user(&user, session)
                .await
                .map_err(|_| AuthenticateError::InternalError)?;

            Ok(DashboardClaims {
                id: user.id,
                sid: user.session_id,
            })
        }
    }
}

impl AuthFacade for CatIdAuthenticator {
    fn convert(
        &self,
        req: &HttpRequest,
    ) -> Result<Box<dyn AuthenticationCredentials>, AuthCredentialsError> {
        let code =
            get_code_from_request(req).ok_or(AuthCredentialsError::InvalidCredentialsFormat)?;

        Ok(Box::new(CatIdCredentials { code }))
    }

    fn get_type(&self) -> String {
        CATID_TYPE_IDENTIFIER.to_string()
    }
}

fn get_code_from_request(req: &HttpRequest) -> Option<String> {
    req.query_string()
        .split('&')
        .find_map(|param| {
            let mut parts = param.split('=');
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                if key == "code" {
                    return Some(value);
                }
            }
            None
        })
        .map(|code| code.to_string())
}

use crate::credentials::AuthenticationCredentials;
use crate::error::{AuthCredentialsError, AuthenticateError};
use crate::token::DashboardClaims;
use crate::{AuthFacade, Authentication};
use actix_web::HttpRequest;
use async_trait::async_trait;
use bcrypt::{hash, DEFAULT_COST};
use data::access::user_access::get_user_from_name;
use scylla::CachingSession;

pub const PEPPER: &str = "x{2G-ki+*";

pub const BASIC_TYPE_IDENTIFIER: &str = "BASIC";
const USERNAME_HEADER: &str = "username";
const PASSWORD_HEADER: &str = "password";

#[derive(Debug)]
pub struct BasicAuthenticator;

#[derive(Debug)]
pub struct BasicCredentials {
    username: String,
    password: String,
}

impl AuthenticationCredentials for BasicCredentials {
    fn get_authentication_identifier(&self) -> String {
        self.password.clone()
    }

    fn get_username(&self) -> Option<String> {
        Some(self.username.clone())
    }
}

impl AuthFacade for BasicAuthenticator {
    fn convert(
        &self,
        req: &HttpRequest,
    ) -> Result<Box<dyn AuthenticationCredentials>, AuthCredentialsError> {
        let username = req
            .headers()
            .get(USERNAME_HEADER)
            .ok_or(AuthCredentialsError::InvalidCredentialsFormat)?
            .to_str()
            .map_err(|_| AuthCredentialsError::InvalidCredentialsFormat)?
            .to_string();
        let password = req
            .headers()
            .get(PASSWORD_HEADER)
            .ok_or(AuthCredentialsError::InvalidCredentialsFormat)?
            .to_str()
            .map_err(|_| AuthCredentialsError::InvalidCredentialsFormat)?
            .to_string();

        Ok(Box::new(BasicCredentials { username, password }))
    }

    fn get_type(&self) -> String {
        BASIC_TYPE_IDENTIFIER.to_string()
    }
}

#[async_trait]
impl Authentication for BasicAuthenticator {
    async fn authenticate(
        &self,
        credentials: Box<dyn AuthenticationCredentials>,
        session: &CachingSession,
    ) -> Result<DashboardClaims, AuthenticateError> {
        let credentials = credentials;

        let user = get_user_from_name(
            credentials
                .get_username()
                .ok_or(AuthenticateError::InvalidCredentials)?,
            session,
        )
        .await
        .map_err(|_| AuthenticateError::InvalidCredentials)?;

        if user.auth_identifier
            != hash(
                credentials.get_authentication_identifier() + PEPPER,
                DEFAULT_COST,
            )
            .map_err(|_| AuthenticateError::InternalError)?
        {
            return Err(AuthenticateError::InvalidCredentials);
        }

        Ok(DashboardClaims {
            id: user.id,
            sid: user.session_id,
        })
    }
}

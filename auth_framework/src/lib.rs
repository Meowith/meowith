use crate::credentials::AuthenticationCredentials;
use crate::error::{AuthCredentialsError, AuthenticateError};
use crate::token::DashboardClaims;
use actix_web::HttpRequest;
use async_trait::async_trait;
use scylla::client::caching_session::CachingSession;
use std::fmt::Debug;

pub mod adapter;
pub mod credentials;
pub mod error;
pub mod token;

#[async_trait]
pub trait Authentication {
    async fn authenticate(
        &self,
        credentials: Box<dyn AuthenticationCredentials>,
        session: &CachingSession,
    ) -> Result<DashboardClaims, AuthenticateError>;
}

pub trait AuthFacade: Authentication + Send + Sync + Debug {
    fn convert(
        &self,
        req: &HttpRequest,
    ) -> Result<Box<dyn AuthenticationCredentials>, AuthCredentialsError>;

    fn get_type(&self) -> String;
}

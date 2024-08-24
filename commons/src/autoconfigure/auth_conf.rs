use data::dto::controller::{
    AuthenticationRequest, AuthenticationResponse, NodeRegisterRequest, NodeRegisterResponse,
    X_ADDR_HEADER,
};
use log::info;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use regex::Regex;
use std::error::Error;
use std::fmt::Debug;
use std::net::IpAddr;
use std::string::ToString;
use std::{env, fs};

use crate::autoconfigure::ssl_conf::perform_certificate_request;
use crate::context::microservice_request_context::MicroserviceRequestContext;
use crate::context::request_context::RequestContext;

static TOKEN_PATH: &str = "tkn.store";

#[derive(Debug, derive_more::Display)]
pub struct TokenReadError {}

impl Error for TokenReadError {}

pub struct RegistrationResult {
    pub internal_cert: X509,
    pub internal_key: PKey<Private>,
}

/// If not already registered, registers the node.
/// Else, the renewal token is read and validated from disk.
/// As well as sets the access_token and renewal_token fields on the ctx.
///
/// Afterward, a new set of SSL certificates is obtained.
/// Lastly, if everything succeeded, the node sends out a health report
///
/// If an mdsftp_error occurs, a panic is issued.
pub async fn register_procedure(
    ctx: &mut MicroserviceRequestContext,
    self_addr: IpAddr,
    token_path: Option<String>,
) -> RegistrationResult {
    let token = read_renewal_token(token_path.clone());
    if let Ok(token) = token {
        info!("Renewal token present");
        ctx.security_context.renewal_token = token;
    } else {
        info!("Performing registration...");
        ctx.security_context.renewal_token = perform_registration(ctx, self_addr).await.unwrap();
        store_renewal_token(token_path, ctx.security_context.renewal_token.clone()).unwrap();
        info!("Done.");
    }
    info!("Fetching access token...");
    let reg_resp = fetch_access_token(ctx)
        .await
        .expect("Failed to fetch the access token");
    ctx.security_context.access_token = reg_resp.access_token;
    ctx.id = reg_resp.id;
    ctx.update_client();
    info!("Fetching certificates...");
    let certificate_pair = perform_certificate_request(ctx, self_addr)
        .await
        .expect("Certificate request failed!");

    info!("Updating health");
    ctx.heartbeat().await.expect("Heartbeat failed");

    info!("Register init done.");
    RegistrationResult {
        internal_cert: certificate_pair.1,
        internal_key: certificate_pair.0,
    }
}

// Note: consider obfuscating the data on disk, or otherwise storing it in a more secure manner.
pub fn store_renewal_token(path: Option<String>, token: String) -> std::io::Result<()> {
    fs::write(path.unwrap_or(TOKEN_PATH.to_string()), token)
}

pub fn read_renewal_token(path: Option<String>) -> Result<String, Box<dyn Error>> {
    let token = fs::read_to_string(path.unwrap_or(TOKEN_PATH.to_string()))?;
    if !is_renewal_token_valid(&token) {
        Err(Box::new(TokenReadError {}))
    } else {
        Ok(token)
    }
}

pub async fn fetch_access_token(
    ctx: &MicroserviceRequestContext,
) -> Result<AuthenticationResponse, Box<dyn Error>> {
    let req = AuthenticationRequest {
        renewal_token: ctx.security_context.renewal_token.clone(),
    };
    Ok(ctx
        .client()
        .await
        .post(ctx.controller("/api/internal/initialize/authenticate"))
        .json(&req)
        .send()
        .await?
        .json::<AuthenticationResponse>()
        .await?)
}

pub async fn perform_registration(
    ctx: &MicroserviceRequestContext,
    addr: IpAddr,
) -> Result<String, Box<dyn Error>> {
    let register_code =
        env::var("REGISTER_CODE").expect("No env var REGISTER_CODE provided. Unable to register!");
    let register_request = NodeRegisterRequest {
        code: register_code,
        service_type: ctx.microservice_type,
    };

    let register_response = ctx
        .client()
        .await
        .post(ctx.controller("/api/internal/initialize/register"))
        .header(X_ADDR_HEADER, addr.to_string())
        .json(&register_request)
        .send()
        .await?
        .json::<NodeRegisterResponse>()
        .await?;

    Ok(register_response.renewal_token)
}

pub fn is_renewal_token_valid(token: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9]{64}$").unwrap();
    re.is_match(token)
}

pub fn is_access_token_valid(token: &str) -> bool {
    // Note. These 2 tokens are the same in their structure.
    is_renewal_token_valid(token)
}

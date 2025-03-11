use std::sync::Arc;

use async_trait::async_trait;
use log::info;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use uuid::Uuid;

use commons::context::controller_request_context::ControllerRequestContext;
use commons::error::protocol_error::{ProtocolError, ProtocolResult};
use logging::log_err;
use protocol::framework::auth::{ConnectionAuthContext, ConnectionAuthenticator};
use protocol::mgpp::server::MGPPServer;

pub async fn start_server(
    port: u16,
    root_certificate: X509,
    authenticator: ControllerAuthenticator,
    cert: (X509, PKey<Private>),
) -> MGPPServer {
    let server = MGPPServer::new(Arc::new(ConnectionAuthContext {
        root_certificate,
        authenticator: Some(Box::new(authenticator)),
        port,
        own_id: Uuid::new_v4(),
    }));
    log_err("MGPP start", server.start_server(port, cert).await);
    info!("MGPP started");
    server
}

pub struct ControllerAuthenticator {
    pub req_ctx: Arc<ControllerRequestContext>,
}

impl ControllerAuthenticator {
    async fn validate_token(&self, node_id: Uuid, req_token: String) -> bool {
        let map = self.req_ctx.node_token.read().await;

        if let Some(token) = map.get(&node_id) {
            *token == req_token
        } else {
            false
        }
    }
}

#[async_trait]
impl ConnectionAuthenticator for ControllerAuthenticator {
    async fn authenticate_outgoing(
        &self,
        _stream: &mut tokio_rustls::TlsStream<TcpStream>,
    ) -> ProtocolResult<()> {
        unreachable!()
    }

    async fn authenticate_incoming(
        &self,
        stream: &mut tokio_rustls::TlsStream<TcpStream>,
        conn_id: Uuid,
    ) -> ProtocolResult<bool> {
        let mut token_buffer = [0u8; 64];
        stream
            .read_exact(&mut token_buffer)
            .await
            .map_err(|_| ProtocolError::AuthenticationFailed)?;

        let validation_response = self
            .validate_token(conn_id, String::from_utf8_lossy(&token_buffer).to_string())
            .await;

        Ok(validation_response)
    }
}

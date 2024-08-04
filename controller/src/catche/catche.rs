use std::sync::Arc;

use async_trait::async_trait;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use uuid::Uuid;

use commons::context::controller_request_context::ControllerRequestContext;
use protocol::catche::catche_server::CatcheServer;
use protocol::mdsftp::authenticator::{ConnectionAuthContext, MeowithConnectionAuthenticator};
use protocol::mdsftp::error::{MDSFTPError, MDSFTPResult};

pub async fn start_server(
    port: u16,
    root_certificate: X509,
    authenticator: ControllerAuthenticator,
    cert: (X509, PKey<Private>),
) {
    let mut server = CatcheServer::new(Arc::new(ConnectionAuthContext {
        root_certificate,
        authenticator: Some(Box::new(authenticator)),
        port,
        own_id: Uuid::new_v4(),
    }));
    let _ = server.start_server(port, cert).await;
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
impl MeowithConnectionAuthenticator for ControllerAuthenticator {
    async fn authenticate_outgoing(
        &self,
        _stream: &mut tokio_rustls::TlsStream<TcpStream>,
    ) -> MDSFTPResult<()> {
        unreachable!()
    }

    async fn authenticate_incoming(
        &self,
        stream: &mut tokio_rustls::TlsStream<TcpStream>,
        conn_id: Uuid,
    ) -> MDSFTPResult<bool> {
        let mut token_buffer = [0u8; 64];
        stream
            .read_exact(&mut token_buffer)
            .await
            .map_err(|_| MDSFTPError::ConnectionAuthenticationError)?;

        let validation_response = self
            .validate_token(conn_id, String::from_utf8_lossy(&token_buffer).to_string())
            .await;

        return Ok(validation_response);
    }
}

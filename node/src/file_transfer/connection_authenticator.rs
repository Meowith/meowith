use std::sync::Arc;

use async_trait::async_trait;
use log::debug;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

use commons::context::microservice_request_context::MicroserviceRequestContext;
use commons::error::protocol_error::{ProtocolError, ProtocolResult};
use protocol::framework::auth::ConnectionAuthenticator;

pub struct MeowithMDSFTPConnectionAuthenticator {
    pub req_ctx: Arc<MicroserviceRequestContext>,
}

#[async_trait]
impl ConnectionAuthenticator for MeowithMDSFTPConnectionAuthenticator {
    async fn authenticate_outgoing(
        &self,
        stream: &mut tokio_rustls::TlsStream<TcpStream>,
    ) -> ProtocolResult<()> {
        stream
            .write_all(self.req_ctx.security_context.access_token.as_bytes())
            .await
            .map_err(|_| ProtocolError::AuthenticationFailed)
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
        let token_str = String::from_utf8_lossy(&token_buffer).to_string();

        let validation_response = self
            .req_ctx
            .validate_peer_token(token_str.clone(), conn_id)
            .await
            .map_err(|_| ProtocolError::AuthenticationFailed);

        if validation_response.is_err() || !validation_response?.valid {
            debug!("authenticate_incoming failed from {conn_id:?}");
            return Ok(false);
        }
        debug!("authenticate_incoming succeeded from {conn_id:?}");
        Ok(true)
    }
}

use async_trait::async_trait;
use commons::context::microservice_request_context::MicroserviceRequestContext;
use protocol::file_transfer::authenticator::MDSFTPConnectionAuthenticator;
use protocol::file_transfer::error::{MDSFTPError, MDSFTPResult};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

pub struct MeowithMDSFTPConnectionAuthenticator {
    req_ctx: Arc<MicroserviceRequestContext>,
}

#[async_trait]
impl MDSFTPConnectionAuthenticator for MeowithMDSFTPConnectionAuthenticator {
    async fn authenticate_outgoing(
        &self,
        stream: &mut tokio_rustls::TlsStream<TcpStream>,
    ) -> MDSFTPResult<()> {
        stream
            .write_all(self.req_ctx.security_context.access_token.as_bytes())
            .await
            .map_err(|_| MDSFTPError::ConnectionAuthenticationError)
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
            .req_ctx
            .validate_peer_token(String::from_utf8_lossy(&token_buffer).to_string(), conn_id)
            .await
            .map_err(|_| MDSFTPError::ConnectionError);

        if validation_response.is_err() || !validation_response.unwrap().valid {
            return Ok(false);
        }

        return Ok(true);
    }
}

use crate::framework::error::ProtocolResult;
use async_trait::async_trait;
use openssl::x509::X509;
use tokio::net::TcpStream;
use tokio_rustls::TlsStream;
use uuid::Uuid;

#[async_trait]
pub trait ConnectionAuthenticator: Send + Sync {
    /// Used when opening a new connection to a remote server.
    async fn authenticate_outgoing(&self, stream: &mut TlsStream<TcpStream>) -> ProtocolResult<()>;

    /// Used upon receiving a connection from a remote host.
    /// Note: the method should not close the connection.
    async fn authenticate_incoming(
        &self,
        stream: &mut TlsStream<TcpStream>,
        conn_id: Uuid,
    ) -> ProtocolResult<bool>;
}

pub struct ConnectionAuthContext {
    pub root_certificate: X509,
    pub authenticator: Option<Box<dyn ConnectionAuthenticator>>,
    pub port: u16,
    pub own_id: Uuid,
}

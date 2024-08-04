use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use rustls::{ClientConfig, RootCertStore};
use rustls::pki_types::{CertificateDer, IpAddr, ServerName};
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, TlsStream};
use uuid::Uuid;

use crate::catche::connection::CatcheConnection;
use crate::catche::error::CatcheError;
use crate::catche::reader::CatchePacketHandler;
use crate::file_transfer::authenticator::ConnectionAuthContext;

pub struct CatcheClient {
    connection: CatcheConnection,
}

impl CatcheClient {
    pub async fn connect(
        addr: &SocketAddr,
        microservice_id: Uuid,
        authenticator: Arc<ConnectionAuthContext>,
        handler: CatchePacketHandler
    ) -> Result<Self, Box<dyn Error>> {
        let mut root_cert_store = RootCertStore::empty();
        root_cert_store
            .add(CertificateDer::from(
                authenticator
                    .root_certificate
                    .to_der()
                    .map_err(|_| CatcheError::SSLError(None))?,
            ))
            .map_err(|_| CatcheError::SSLError(None))?;
        let config = ClientConfig::builder()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let server_name = ServerName::IpAddress(IpAddr::from(addr.ip()));

        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|_| CatcheError::ConnectionError)?;

        let stream = TlsStream::from(
            connector
                .connect(server_name, stream)
                .await
                .map_err(|_| CatcheError::SSLError(None))?,
        );

        let client = CatcheClient {
            connection: CatcheConnection::from_conn(stream, handler).await?
        };

        client.connection.write_auth_header(microservice_id).await?;

        Ok(client)
    }

    pub async fn write_invalidate_packet(&self) -> std::io::Result<()> {
        self.connection.write_invalidate_packet().await
    }
}
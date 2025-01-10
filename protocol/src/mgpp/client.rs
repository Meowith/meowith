use openssl::x509::X509;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::{Arc};
use rustls::pki_types::{CertificateDer, IpAddr, ServerName};
use rustls::{ClientConfig, RootCertStore};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::{TlsConnector, TlsStream};
use uuid::Uuid;
use crate::framework::connection::ProtocolConnection;
use crate::framework::error::ProtocolResult;
use crate::framework::writer::PacketWriter;
use crate::mgpp::error::MGPPError;
use crate::mgpp::handler::{MGPPHandlers, MGPPHandlersMapper};
use crate::mgpp::packet::{MGPPPacket, MGPPPacketDispatcher, MGPPPacketSerializer};

#[derive(Clone)]
pub struct MGPPClient {
    connection: Arc<ProtocolConnection<MGPPPacket>>,
}

impl MGPPClient {
    pub async fn connect(
        addr: &SocketAddr,
        microservice_id: Uuid,
        root_certificate: X509,
        token: Option<String>,
        handlers: MGPPHandlers
    ) -> Result<Self, Box<dyn Error>> {
        let mut root_cert_store = RootCertStore::empty();
        root_cert_store
            .add(CertificateDer::from(
                root_certificate
                    .to_der()
                    .map_err(|_| MGPPError::SSLError(None))?,
            ))
            .map_err(|_| MGPPError::SSLError(None))?;
        let config = ClientConfig::builder()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let server_name = ServerName::IpAddress(IpAddr::from(addr.ip()));

        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| MGPPError::SSLError(Some(e.into())))?;

        let stream = TlsStream::from(
            connector
                .connect(server_name, stream)
                .await
                .map_err(|_| MGPPError::SSLError(None))?,
        );

        let (read, write) = tokio::io::split(stream);
        let writer = Arc::new(Mutex::new(PacketWriter::new(write, Arc::new(MGPPPacketSerializer))));

        let client = MGPPClient {
            connection: Arc::new(ProtocolConnection::new(read, Arc::new(MGPPPacketDispatcher {
                handler: Box::new(MGPPHandlersMapper::new(handlers)),
                writer: Arc::downgrade(&writer),
            }), writer))
        };
        let writer = client.connection.0.obtain_writer();
        // write auth header
        writer.lock().await.write(microservice_id.as_bytes()).await?;

        if let Some(token) = token {
            writer.lock().await.write(token.as_bytes()).await?;
        }

        Ok(client)
    }

    pub async fn write_packet(
        &self,
        packet: MGPPPacket,
    ) -> ProtocolResult<()> {
        self.connection
            .0
            .obtain_writer()
            .lock()
            .await
            .write_packet(packet)
            .await
    }

    pub async fn shutdown(&self) -> ProtocolResult<()> {
        self.connection.0.shutdown().await
    }
}

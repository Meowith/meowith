use crate::framework::connection::ProtocolConnection;
use crate::framework::error::ProtocolResult;
use crate::framework::writer::PacketWriter;
use crate::mgpp::error::MGPPError;
use crate::mgpp::handler::{MGPPHandlers, MGPPHandlersMapper};
use crate::mgpp::packet::{MGPPPacket, MGPPPacketDispatcher, MGPPPacketSerializer};
use commons::pause_handle::ApplicationPauseHandle;
use log::info;
use openssl::x509::X509;
use rustls::pki_types::{CertificateDer, IpAddr, ServerName};
use rustls::{ClientConfig, RootCertStore};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_rustls::{TlsConnector, TlsStream};
use uuid::Uuid;

#[derive(Clone)]
pub struct MGPPClient {
    connection: Arc<Mutex<ProtocolConnection<MGPPPacket>>>,
    mgpp_connection_config: Arc<MGPPConnectionConfig>,
    handlers: Arc<MGPPHandlers>,
}

struct MGPPConnectionConfig {
    addr: SocketAddr,
    root_certificate: X509,
    microservice_id: Uuid,
    token: Option<String>,
}

impl MGPPClient {
    pub async fn connect(
        addr: SocketAddr,
        root_certificate: X509,
        microservice_id: Uuid,
        token: Option<String>,
        handlers: MGPPHandlers,
    ) -> Result<Self, Box<dyn Error>> {
        let handlers = Arc::new(handlers);
        let mgpp_config = Arc::new(MGPPConnectionConfig {
            addr,
            root_certificate,
            microservice_id,
            token,
        });
        let connection = Arc::new(Mutex::new(
            MGPPClient::create_connection(mgpp_config.clone(), handlers.clone()).await?,
        ));
        Ok(Self {
            connection,
            mgpp_connection_config: mgpp_config,
            handlers,
        })
    }

    pub async fn setup_auto_reconnect(&self, pause_handle: Arc<Box<dyn ApplicationPauseHandle>>) {}

    #[allow(dead_code)]
    async fn create_watcher(
        &self,
        mgpp_config: Arc<MGPPConnectionConfig>,
        handlers: Arc<MGPPHandlers>,
        connection: Arc<Mutex<ProtocolConnection<MGPPPacket>>>,
    ) {
        tokio::spawn(async move {
            let connection_guard = connection.lock().await;
            let shutdown_receiver = connection_guard.shutdown_receiver.clone();
            drop(connection_guard);

            if shutdown_receiver.lock().await.recv().await.is_some() {
                info!("Restarting the mgpp connection due tu unexpected closure in 3 seconds");
                sleep(Duration::from_secs(3)).await;
                let _new_connection = MGPPClient::create_connection(mgpp_config, handlers).await;
                // TODO
            }
            // else the connection has been closed willfully and shall not be reopened
        });
    }

    async fn create_connection(
        mgpp_config: Arc<MGPPConnectionConfig>,
        handlers: Arc<MGPPHandlers>,
    ) -> Result<ProtocolConnection<MGPPPacket>, Box<dyn Error>> {
        let mut root_cert_store = RootCertStore::empty();
        root_cert_store
            .add(CertificateDer::from(
                mgpp_config
                    .root_certificate
                    .to_der()
                    .map_err(|_| MGPPError::SSLError(None))?,
            ))
            .map_err(|_| MGPPError::SSLError(None))?;
        let config = ClientConfig::builder()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let server_name = ServerName::IpAddress(IpAddr::from(mgpp_config.addr.ip()));

        let stream = TcpStream::connect(&mgpp_config.addr)
            .await
            .map_err(|e| MGPPError::SSLError(Some(e.into())))?;

        let sock_ref = socket2::SockRef::from(&stream);

        let mut ka = socket2::TcpKeepalive::new();
        ka = ka.with_time(Duration::from_secs(20));
        ka = ka.with_interval(Duration::from_secs(20));

        sock_ref.set_tcp_keepalive(&ka)?;

        let stream = TlsStream::from(
            connector
                .connect(server_name, stream)
                .await
                .map_err(|_| MGPPError::SSLError(None))?,
        );

        let (read, write) = tokio::io::split(stream);
        let writer = Arc::new(Mutex::new(PacketWriter::new(
            write,
            Arc::new(MGPPPacketSerializer),
        )));

        let dispatcher = Arc::new(MGPPPacketDispatcher {
            handler: Box::new(MGPPHandlersMapper::new(handlers)),
            writer: Arc::downgrade(&writer),
        });

        let connection = ProtocolConnection::new(read, dispatcher, writer);

        connection
            .write(mgpp_config.microservice_id.as_bytes())
            .await?;
        if let Some(token) = mgpp_config.token.clone() {
            connection.write(token.as_bytes()).await?;
        }

        Ok(connection)
    }

    pub async fn write_packet(&self, packet: MGPPPacket) -> ProtocolResult<()> {
        self.connection.lock().await.write_packet(packet).await
    }

    pub async fn shutdown(&self) -> ProtocolResult<()> {
        self.connection.lock().await.shutdown(false).await
    }
}

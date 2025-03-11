use crate::framework::connection::ProtocolConnection;
use crate::framework::writer::PacketWriter;
use crate::mgpp::error::MGPPError;
use crate::mgpp::handler::{MGPPHandlers, MGPPHandlersMapper};
use crate::mgpp::packet::{MGPPPacket, MGPPPacketDispatcher, MGPPPacketSerializer};
use commons::error::protocol_error::ProtocolResult;
use commons::pause_handle::ApplicationPauseHandle;
use log::{info, warn};
use openssl::x509::X509;
use rustls::pki_types::{CertificateDer, IpAddr, ServerName};
use rustls::{ClientConfig, RootCertStore};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_rustls::{TlsConnector, TlsStream};
use uuid::Uuid;

struct ConnectionHolder {
    pub connection: ProtocolConnection<MGPPPacket>,
    pub reconnects: AtomicUsize,
}

#[derive(Clone)]
pub struct MGPPClient {
    connection: Arc<Mutex<ConnectionHolder>>,
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
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let handlers = Arc::new(handlers);
        let mgpp_config = Arc::new(MGPPConnectionConfig {
            addr,
            root_certificate,
            microservice_id,
            token,
        });
        let connection = Arc::new(Mutex::new(ConnectionHolder {
            connection: MGPPClient::create_connection(mgpp_config.clone(), handlers.clone())
                .await?,
            reconnects: AtomicUsize::new(0),
        }));
        Ok(Self {
            connection,
            mgpp_connection_config: mgpp_config,
            handlers,
        })
    }

    #[allow(dead_code)]
    pub async fn set_up_auto_reconnect(&self, pause_handle: Arc<Box<dyn ApplicationPauseHandle>>) {
        let connection = self.connection.clone();
        let mgpp_config = self.mgpp_connection_config.clone();
        let handlers = self.handlers.clone();

        tokio::spawn(async move {
            let connection = connection.clone();
            
            info!("Starting up mgpp watchdog");
            
            loop {
                let shutdown_receiver = {
                    let connection_guard = connection.lock().await;
                    connection_guard.connection.shutdown_receiver.clone()
                };

                if shutdown_receiver.lock().await.recv().await.is_some() {
                    loop {
                        pause_handle.pause().await;
                        info!(
                            "Restarting the mgpp connection due tu unexpected closure in 1 second"
                        );
                        sleep(Duration::from_secs(1)).await;

                        if Arc::strong_count(&connection) <= 1 {
                            return; // only this task holds a ref, meaning the client has been dropped.
                        }
                        
                        info!("Attempting reconnect...");
                        let new_connection =
                            MGPPClient::create_connection(mgpp_config.clone(), handlers.clone())
                                .await;

                        if let Ok(new_connection) = new_connection {
                            if Arc::strong_count(&connection) <= 1 {
                                return;
                            }

                            info!("MGGP reconnected");
                            let mut conn = connection.lock().await;
                            conn.connection = new_connection;
                            conn.reconnects.fetch_add(1, Ordering::Relaxed);
                            pause_handle.resume().await;

                            break;
                        } else {
                            warn!("Reconnect attempt error: {:?}", new_connection.unwrap_err());
                        }
                    }
                } // else the connection has been closed willfully and shall not be reopened
            }
        });
    }

    async fn create_connection(
        mgpp_config: Arc<MGPPConnectionConfig>,
        handlers: Arc<MGPPHandlers>,
    ) -> Result<ProtocolConnection<MGPPPacket>, Box<dyn Error + Send + Sync>> {
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

        // Note: the authenticator should probably do this
        let (read, mut write) = tokio::io::split(stream);
        write
            .write_all(mgpp_config.microservice_id.as_bytes())
            .await?;
        if let Some(token) = mgpp_config.token.clone() {
            write.write_all(token.as_bytes()).await?;
        }

        let writer = Arc::new(Mutex::new(PacketWriter::new(
            write,
            Arc::new(MGPPPacketSerializer),
        )));

        let dispatcher = Arc::new(MGPPPacketDispatcher {
            handler: Box::new(MGPPHandlersMapper::new(handlers)),
            writer: Arc::downgrade(&writer),
        });

        let connection = ProtocolConnection::new(read, dispatcher, writer);

        Ok(connection)
    }

    pub async fn write_packet(&self, packet: MGPPPacket) -> ProtocolResult<()> {
        self.connection
            .lock()
            .await
            .connection
            .write_packet(packet)
            .await
    }

    pub async fn shutdown(&self) -> ProtocolResult<()> {
        self.connection
            .lock()
            .await
            .connection
            .shutdown(false)
            .await
    }

    pub async fn get_reconnects(&self) -> usize {
        self.connection
            .lock()
            .await
            .reconnects
            .load(Ordering::Relaxed)
    }
}

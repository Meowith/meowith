use std::any::Any;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::framework::error::ProtocolError;
use crate::framework::server_wire::handle_incoming_connection;
use async_trait::async_trait;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::Sender;
use tokio::sync::{broadcast, oneshot, Mutex};
use tokio_rustls::server::TlsStream;
use tokio_rustls::{rustls, TlsAcceptor};
use uuid::Uuid;

/// Trait that defines a protocol that the server can handle
#[async_trait]
pub trait Protocol: Send + Sync + 'static {
    /// Called to handle an incoming connection.
    async fn handle_connection(&self, connection: ProtocolConnection) -> Result<(), ProtocolError>;

    /// Allows downcasting of the trait.
    fn as_any(&self) -> &dyn Any;
}

/// Represents a single connection
pub struct ProtocolConnection {
    pub stream: TlsStream<TcpStream>,
    pub id: Uuid,
}

impl ProtocolConnection {
    pub async fn send(&mut self, data: &[u8]) -> io::Result<()> {
        self.stream.write_all(data).await
    }

    pub async fn receive(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buffer).await
    }
}

#[derive(Clone)]
pub struct ProtocolServer {
    internal: Arc<InternalProtocolServer>,
}

pub struct InternalProtocolServer {
    running: Arc<AtomicBool>,
    connections: Arc<Mutex<Vec<ProtocolConnection>>>,
    shutdown_sender: Arc<Mutex<Option<Sender<()>>>>,
    protocol_handler: Arc<dyn Protocol>,
}

impl ProtocolServer {
    /// Create a new server instance
    pub fn new(protocol_handler: Arc<dyn Protocol>) -> Self {
        ProtocolServer {
            internal: Arc::new(InternalProtocolServer::new(protocol_handler)),
        }
    }

    /// Starts the server on the given port using TLS certificates.
    pub async fn start(&self, port: u16, cert: (X509, PKey<Private>)) -> io::Result<()> {
        self.internal.start_server(port, cert).await
    }

    /// Shutdowns the server gracefully.
    pub async fn shutdown(&self) {
        self.internal.shutdown().await;
    }
}

impl InternalProtocolServer {
    pub fn new(protocol_handler: Arc<dyn Protocol>) -> Self {
        InternalProtocolServer {
            running: Arc::new(AtomicBool::new(false)),
            shutdown_sender: Arc::new(Mutex::new(None)),
            connections: Arc::new(Mutex::new(Vec::new())),
            protocol_handler,
        }
    }

    /// Starts the server
    pub async fn start_server(&self, port: u16, cert: (X509, PKey<Private>)) -> io::Result<()> {
        // Configure the TLS acceptor
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(cert.0.to_der()?)],
                PrivateKeyDer::try_from(cert.1.private_key_to_der()?)
                    .map_err(|_| ProtocolError::AuthenticationFailed)?,
            )
            .map_err(|_| ProtocolError::AuthenticationFailed)?;

        let acceptor = TlsAcceptor::from(Arc::new(config));

        // Create a TCP listener
        let listener =
            TcpListener::bind(SocketAddr::new(IpAddr::from_str("0.0.0.0").unwrap(), port)).await?;

        self.running.store(true, Ordering::SeqCst);

        // Broadcast channel for shutdown signal
        let (shutdown_tx, _) = broadcast::channel(1);
        *self.shutdown_sender.lock().await = Some(shutdown_tx);

        let running = self.running.clone();
        let shutdown_sender = self.shutdown_sender.clone();
        let protocol_handler = self.protocol_handler.clone();

        let (startup_tx, startup_rx) = oneshot::channel();

        tokio::spawn(async move {
            // Notify that the server has started
            let _ = startup_tx.send(());

            // Run the server loop
            while running.load(Ordering::Relaxed) {
                if let Err(err) = handle_incoming_connection(
                    &acceptor,
                    &listener,
                    &shutdown_sender,
                    &running,
                    protocol_handler.clone(),
                )
                .await
                {
                    if matches!(err, ProtocolError::ShuttingDown) {
                        break; // Gracefully exit
                    } else {
                        eprintln!("Error while handling connection: {:?}", err);
                    }
                }
            }
        });

        let _ = startup_rx.await; // Ensure the server is fully initialized before returning
        Ok(())
    }

    /// Shutdown the server
    pub async fn shutdown(&self) {
        let mut lock = self.shutdown_sender.lock().await;
        if let Some(sender) = lock.as_mut() {
            self.running.store(false, Ordering::SeqCst);
            let _ = sender.send(());
        }
    }
}

impl Drop for InternalProtocolServer {
    fn drop(&mut self) {
        let running = self.running.clone();
        let sender = self.shutdown_sender.clone();

        tokio::spawn(async move {
            let mut lock = sender.lock().await;
            if let Some(sender) = lock.as_mut() {
                running.store(false, Ordering::SeqCst);
                let _ = sender.send(());
            }
        });
    }
}

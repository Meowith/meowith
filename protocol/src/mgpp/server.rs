use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::framework::connection::ProtocolConnection;
use crate::framework::writer::PacketWriter;
use crate::mdsftp::authenticator::ConnectionAuthContext;
use crate::mdsftp::server::ZERO_UUID;
use crate::mgpp::error::MGPPError;
use crate::mgpp::handler::{MGPPHandlers, MGPPHandlersMapper};
use crate::mgpp::packet::{MGPPPacket, MGPPPacketDispatcher, MGPPPacketSerializer};
use crate::mgpp::server_handlers::MGPPServerCacheInvalidateHandler;
use crate::mgpp::MGPPConnection;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::{broadcast, oneshot, Mutex};
use tokio_rustls::{rustls, TlsAcceptor, TlsStream};
use uuid::{Bytes, Uuid};

#[derive(Clone)]
pub struct MGPPServer {
    _internal: Arc<InternalMGPPServer>,
}

pub struct InternalMGPPServer {
    running: Arc<AtomicBool>,
    connections: Arc<Mutex<Vec<MGPPConnection>>>,
    connection_auth_context: Arc<ConnectionAuthContext>,
    shutdown_sender: Arc<Mutex<Option<Sender<()>>>>,
}

impl MGPPServer {
    pub async fn broadcast_packet(&self, packet: MGPPPacket) -> Result<(), MGPPError> {
        let connections = self._internal.connections.lock().await;

        for connection in &*connections {
            connection
                .write_packet(packet.clone())
                .await
                .map_err(|_| MGPPError::ConnectionError)?;
        }

        Ok(())
    }

    pub fn new(connection_auth_context: Arc<ConnectionAuthContext>) -> Self {
        MGPPServer {
            _internal: Arc::new(InternalMGPPServer::new(connection_auth_context)),
        }
    }

    pub async fn start_server(&self, port: u16, cert: (X509, PKey<Private>)) -> io::Result<()> {
        self._internal.start_server(port, cert).await
    }

    pub async fn shutdown(&self) {
        self._internal.shutdown().await
    }
}

impl InternalMGPPServer {
    pub fn new(connection_auth_context: Arc<ConnectionAuthContext>) -> Self {
        InternalMGPPServer {
            running: Arc::new(AtomicBool::new(false)),
            shutdown_sender: Arc::new(Mutex::new(None)),
            connections: Arc::new(Mutex::new(Vec::new())),
            connection_auth_context,
        }
    }

    pub async fn start_server(&self, port: u16, cert: (X509, PKey<Private>)) -> io::Result<()> {
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(cert.0.to_der().unwrap())],
                PrivateKeyDer::try_from(cert.1.private_key_to_der().unwrap()).unwrap(),
            )
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener =
            TcpListener::bind(SocketAddr::new(IpAddr::from_str("0.0.0.0").unwrap(), port)).await?;
        let auth_ctx = self.connection_auth_context.clone();
        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();

        let (tx, _rx) = broadcast::channel(1);
        *self.shutdown_sender.lock().await = Some(tx);
        let shutdown_sender = self.shutdown_sender.clone();
        let connections = self.connections.clone();

        let (startup_tx, startup_rx) = oneshot::channel();

        tokio::spawn(async move {
            startup_tx.send(()).unwrap();

            while running.load(Ordering::Relaxed) {
                let res: Result<(), MGPPError> = async {
                    let stream: TcpStream;
                    let mut rx: Receiver<()>;
                    {
                        let mut a = shutdown_sender.lock().await;
                        let tx = a.as_mut().unwrap();
                        rx = tx.subscribe();
                    }

                    if !running.load(Ordering::Relaxed) {
                        return Err(MGPPError::ShuttingDown);
                    }

                    tokio::select! {
                        _val = rx.recv() => {
                            return Err(MGPPError::ShuttingDown);
                        }
                        val = listener.accept() => {
                            stream = val.map_err(|_| MGPPError::ConnectionError)?.0
                        }
                    }

                    let acceptor = acceptor.clone();
                    let stream = acceptor
                        .accept(stream)
                        .await
                        .map_err(|_| MGPPError::ConnectionError)?;

                    let mut stream = TlsStream::from(stream);

                    let mut auth_header: [u8; 16] = [0; 16];

                    if stream.read_exact(&mut auth_header).await.is_err() {
                        stream
                            .shutdown()
                            .await
                            .map_err(|_| MGPPError::ConnectionError)?;
                        return Err(MGPPError::ConnectionError);
                    }

                    let microservice_id =
                        Uuid::from_bytes(Bytes::try_from(auth_header).unwrap_or(ZERO_UUID));

                    if let Some(auth) = &auth_ctx.authenticator {
                        if !auth
                            .authenticate_incoming(&mut stream, microservice_id)
                            .await
                            .map_err(|_| MGPPError::ConnectionAuthenticationError)?
                        {
                            stream
                                .shutdown()
                                .await
                                .map_err(|_| MGPPError::ConnectionAuthenticationError)?;
                            return Err(MGPPError::ConnectionAuthenticationError);
                        }
                    }

                    let connections_clone = connections.clone();

                    let (read, write) = tokio::io::split(stream);
                    let writer = Arc::new(Mutex::new(PacketWriter::new(
                        write,
                        Arc::new(MGPPPacketSerializer),
                    )));
                    let handlers = Arc::new(MGPPHandlers {
                        invalidate_cache: Box::new(MGPPServerCacheInvalidateHandler {
                            connections: connections_clone,
                        }),
                    });

                    connections.lock().await.push(ProtocolConnection::new(
                        read,
                        Arc::new(MGPPPacketDispatcher {
                            handler: Box::new(MGPPHandlersMapper::new(handlers)),
                            writer: Arc::downgrade(&writer),
                        }),
                        writer,
                    ));

                    Ok(())
                }
                .await;

                match res {
                    Ok(_) => {}
                    Err(MGPPError::ShuttingDown) => {
                        break;
                    }
                    Err(_) => {}
                }
            }
        });

        let _ = startup_rx.await;
        Ok(())
    }

    pub async fn shutdown(&self) {
        let sender = self.shutdown_sender.clone();
        let mut lock = sender.lock().await;
        if let Some(sender) = lock.as_mut() {
            self.running.store(false, Ordering::SeqCst);
            let _ = sender.send(());
        }
    }
}

impl Drop for InternalMGPPServer {
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

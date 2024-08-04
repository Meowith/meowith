use std::any::Any;
use crate::catche::error::CatcheError;
use crate::file_transfer::authenticator::ConnectionAuthContext;
use crate::file_transfer::server::ZERO_UUID;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use async_trait::async_trait;
use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::{broadcast, oneshot, Mutex};
use tokio_rustls::{rustls, TlsAcceptor, TlsStream};
use uuid::{Bytes, Uuid};
use crate::catche::connection::CatcheConnection;
use crate::catche::handler::CatcheHandler;

#[allow(unused)]
pub struct CatcheServer {
    running: Arc<AtomicBool>,
    connections: Arc<Mutex<Vec<CatcheConnection>>>,
    connection_auth_context: Arc<ConnectionAuthContext>,
    shutdown_sender: Option<Arc<Mutex<Sender<()>>>>,
}

#[derive(Clone, Debug)]
pub struct CatcheServerHandler {
    connections: Arc<Mutex<Vec<CatcheConnection>>>,
}

#[async_trait]
impl CatcheHandler for CatcheServerHandler {
    #[allow(clippy::unnecessary_to_owned)]
    async fn handle_invalidate(&mut self) {
        let conns = self.connections.lock().await;

        for connection in conns.to_vec() {
            let _ = connection.write_invalidate_packet().await;
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl CatcheServer {
    #[allow(unused)]
    pub fn new(connection_auth_context: Arc<ConnectionAuthContext>) -> Self {
        CatcheServer {
            running: Arc::new(AtomicBool::new(false)),
            shutdown_sender: None,
            connections: Arc::new(Mutex::new(Vec::new())),
            connection_auth_context,
        }
    }

    pub async fn start_server(&mut self, port: u16, cert: (X509, PKey<Private>)) -> io::Result<()> {
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
        let shutdown_sender = Arc::new(Mutex::new(tx));
        self.shutdown_sender = Some(shutdown_sender.clone());
        let connections = self.connections.clone();

        let (startup_tx, startup_rx) = oneshot::channel();

        tokio::spawn(async move {
            startup_tx.send(()).unwrap();

            while running.load(Ordering::Relaxed) {
                let res: Result<(), CatcheError> = async {
                    let stream: TcpStream;
                    let mut rx: Receiver<()>;
                    {
                        let tx = shutdown_sender.lock().await;
                        rx = tx.subscribe();
                    }

                    if !running.load(Ordering::Relaxed) {
                        return Err(CatcheError::ShuttingDown);
                    }

                    tokio::select! {
                        _val = rx.recv() => {
                            return Err(CatcheError::ShuttingDown);
                        }
                        val = listener.accept() => {
                            stream = val.map_err(|_| CatcheError::ConnectionError)?.0
                        }
                    }

                    let acceptor = acceptor.clone();
                    let stream = acceptor
                        .accept(stream)
                        .await
                        .map_err(|_| CatcheError::ConnectionError)?;

                    let mut stream = TlsStream::from(stream);

                    let mut auth_header: [u8; 16] = [0; 16];

                    if stream.read_exact(&mut auth_header).await.is_err() {
                        stream
                            .shutdown()
                            .await
                            .map_err(|_| CatcheError::ConnectionError)?;
                        return Err(CatcheError::ConnectionError);
                    }

                    let microservice_id =
                        Uuid::from_bytes(Bytes::try_from(auth_header).unwrap_or(ZERO_UUID));

                    if let Some(auth) = &auth_ctx.authenticator {
                        if !auth
                            .authenticate_incoming(&mut stream, microservice_id)
                            .await
                            .map_err(|_| CatcheError::ConnectionAuthenticationError)?
                        {
                            stream
                                .shutdown()
                                .await
                                .map_err(|_| CatcheError::ConnectionAuthenticationError)?;
                            return Err(CatcheError::ConnectionAuthenticationError);
                        }
                    }

                    let connections_clone = connections.clone();

                    connections.lock().await.push(
                        CatcheConnection::from_conn(stream, Arc::new(Mutex::new(Box::new(
                            CatcheServerHandler {
                                connections: connections_clone,
                            }
                        )))).await?
                    );

                    Ok(())
                }
                .await;

                match res {
                    Ok(_) => {}
                    Err(CatcheError::ShuttingDown) => {
                        break;
                    }
                    Err(_) => {}
                }
            }
        });

        let _ = startup_rx.await;
        Ok(())
    }

    #[allow(unused)]
    pub async fn shutdown(self) {
        let sender = self.shutdown_sender.clone();
        if let Some(sender) = sender {
            self.running.store(false, Ordering::SeqCst);
            let _ = sender.lock().await.send(());
        }
    }
}

impl Drop for CatcheServer {
    fn drop(&mut self) {
        let running = self.running.clone();
        let sender = self.shutdown_sender.clone();

        tokio::spawn(async move {
            if let Some(mut_sender) = sender {
                running.store(false, Ordering::SeqCst);

                let _ = mut_sender.lock().await.send(());
            }
        });
    }
}

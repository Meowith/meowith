use std::error::Error;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use log::debug;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::{broadcast, oneshot, Mutex};
use tokio_rustls::{rustls, TlsAcceptor, TlsStream};
use uuid::{Bytes, Uuid};

use commons::context::microservice_request_context::NodeAddrMap;

use crate::mdsftp::authenticator::ConnectionAuthContext;
use crate::mdsftp::error::MDSFTPError;
use crate::mdsftp::pool::{MDSFTPPool, MDSFTPPoolConfigHolder, PacketHandlerRef};

pub const ZERO_UUID: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

#[allow(unused)]
pub struct MDSFTPServer {
    pool: Option<MDSFTPPool>,
    running: Arc<AtomicBool>,
    connection_auth_context: Arc<ConnectionAuthContext>,
    node_addr_map: NodeAddrMap,
    cfg: MDSFTPPoolConfigHolder,
    shutdown_sender: Option<Arc<Mutex<Sender<()>>>>,
}

impl MDSFTPServer {
    pub async fn new(
        connection_auth_context: Arc<ConnectionAuthContext>,
        node_addr_map: NodeAddrMap,
        incoming_handler: PacketHandlerRef,
        cfg: MDSFTPPoolConfigHolder,
    ) -> Self {
        let mut srv = MDSFTPServer {
            pool: None,
            connection_auth_context,
            node_addr_map,
            running: Arc::new(AtomicBool::new(false)),
            shutdown_sender: None,
            cfg,
        };
        srv.create_pool(incoming_handler).await;
        srv
    }

    #[allow(unused)]
    pub async fn start(&mut self, cert: &X509, key: &PKey<Private>) -> Result<(), Box<dyn Error>> {
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(cert.to_der().unwrap())],
                PrivateKeyDer::try_from(key.private_key_to_der().unwrap()).unwrap(),
            )
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener = TcpListener::bind(SocketAddr::new(
            IpAddr::from_str("0.0.0.0").unwrap(),
            self.connection_auth_context.port,
        ))
        .await?;
        let auth_ctx = self.connection_auth_context.clone();

        let pool = self.pool.clone().ok_or(MDSFTPError::NoPool)?;
        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();

        let (tx, _rx) = broadcast::channel(1);
        let shutdown_sender = Arc::new(Mutex::new(tx));
        self.shutdown_sender = Some(shutdown_sender.clone());

        let (startup_tx, startup_rx) = oneshot::channel();

        tokio::spawn(async move {
            startup_tx.send(());
            while running.load(Ordering::Relaxed) {
                let res: Result<(), MDSFTPError> = async {
                    let stream: TcpStream;
                    let mut rx: Receiver<()>;
                    {
                        let tx = shutdown_sender.lock().await;
                        rx = tx.subscribe();
                    }

                    // Send calls should work now, check running again just in case.
                    if !running.load(Ordering::Relaxed) {
                        debug!("Fast shutdown");
                        return Err(MDSFTPError::ShuttingDown);
                    }

                    tokio::select! {
                        val = rx.recv() => {
                            return Err(MDSFTPError::ShuttingDown);
                        }
                        val = listener.accept() => {
                            stream = val.map_err(|_| MDSFTPError::ConnectionError)?.0
                        }
                    }

                    let acceptor = acceptor.clone();
                    let stream = acceptor
                        .accept(stream)
                        .await
                        .map_err(|_| MDSFTPError::ConnectionError)?;

                    let mut stream = TlsStream::from(stream);

                    debug!("Reading metadata");

                    // read 16byte id
                    let mut auth_header: [u8; 16] = [0; 16];

                    if stream.read_exact(&mut auth_header).await.is_err() {
                        stream
                            .shutdown()
                            .await
                            .map_err(|_| MDSFTPError::ConnectionError)?;
                        return Err(MDSFTPError::ConnectionError);
                    }

                    let microservice_id =
                        Uuid::from_bytes(Bytes::try_from(auth_header).unwrap_or(ZERO_UUID));

                    if let Some(auth) = &auth_ctx.authenticator {
                        debug!("Validating new MDSFTP remote connection...");
                        if (!auth
                            .authenticate_incoming(&mut stream, microservice_id)
                            .await?)
                        {
                            debug!("Validation unsuccessful");
                            stream
                                .shutdown()
                                .await
                                .map_err(|_| MDSFTPError::ConnectionAuthenticationError)?;
                            return Err(MDSFTPError::ConnectionAuthenticationError);
                        }
                    } else {
                        debug!("No validator, skipping.")
                    }

                    debug!("Adding a new remote connection");
                    pool._internal_pool
                        .lock()
                        .await
                        .add_connection(microservice_id, stream)
                        .await?;

                    Ok(())
                }
                .await;

                match res {
                    Ok(_) => {}
                    Err(MDSFTPError::ShuttingDown) => {
                        break;
                    }
                    Err(_) => {}
                }
            }
        });

        let _ = startup_rx.await;
        Ok(())
    }

    pub fn pool(&self) -> MDSFTPPool {
        self.pool.clone().unwrap()
    }

    async fn create_pool(&mut self, incoming_handler: PacketHandlerRef) {
        let mut pool = MDSFTPPool::new(
            self.connection_auth_context.clone(),
            self.node_addr_map.clone(),
            self.cfg.clone(),
        );
        pool.set_packet_handler(incoming_handler).await;
        self.pool = Some(pool)
    }

    pub async fn shutdown(self) {
        let sender = self.shutdown_sender.clone();
        if let Some(sender) = sender {
            self.running.store(false, Ordering::SeqCst);
            let _ = sender.lock().await.send(());
        }
    }
}

impl Drop for MDSFTPServer {
    fn drop(&mut self) {
        let sender = self.shutdown_sender.clone();
        let running = self.running.clone();
        tokio::spawn(async move {
            if let Some(sender) = sender {
                running.store(false, Ordering::SeqCst);
                let _ = sender.lock().await.send(());
            }
        });
    }
}

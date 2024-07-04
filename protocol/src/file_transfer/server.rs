use log::debug;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::error::Error;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::{rustls, TlsAcceptor, TlsStream};
use uuid::{Bytes, Uuid};

use commons::context::microservice_request_context::MicroserviceRequestContext;

use crate::file_transfer::error::MDSFTPError;
use crate::file_transfer::handler::PacketHandler;
use crate::file_transfer::pool::MDSFTPPool;

#[allow(unused)]
const ZERO_UUID: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

#[allow(unused)]
pub struct MDSFTPServer {
    pool: Option<MDSFTPPool>,
    req_ctx: Arc<MicroserviceRequestContext>,
    running: Arc<AtomicBool>
}

impl MDSFTPServer {
    pub async fn new(req_ctx: Arc<MicroserviceRequestContext>, incoming_handler: Box<dyn PacketHandler>) -> Self {
        let mut srv = MDSFTPServer {
            pool: None,
            req_ctx,
            running: Arc::new(AtomicBool::new(false)),
        };
        srv.create_pool(incoming_handler).await;
        srv
    }

    #[allow(unused)]
    async fn start(&self, cert: &X509, key: &PKey<Private>) -> Result<(), Box<dyn Error>> {
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(cert.to_der().unwrap())],
                PrivateKeyDer::try_from(key.private_key_to_der().unwrap()).unwrap(),
            )
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener = TcpListener::bind(SocketAddr::new(IpAddr::from_str("0.0.0.0").unwrap(), self.req_ctx.port_configuration.mdsftp_server_port)).await?;
        let req_ctx = self.req_ctx.clone();

        let pool = self.pool.clone().ok_or(MDSFTPError::NoPool)?;
        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();

        tokio::spawn(async move {
            while running.load(Ordering::Relaxed) {
                let _: Result<(), MDSFTPError> = async {
                    let (stream, _) = listener
                        .accept()
                        .await
                        .map_err(|_| MDSFTPError::SSLError)?;
                    let acceptor = acceptor.clone();
                    let stream = acceptor
                        .accept(stream)
                        .await
                        .map_err(|_| MDSFTPError::SSLError)?;

                    let mut stream = TlsStream::from(stream);

                    debug!("Validating new MDSFTP remote connection...");
                    // read 64char token + 16byte id
                    let mut auth_header: [u8; 64 + 16] = [0; 64 + 16];

                    if stream.read_exact(&mut auth_header).await.is_err() {
                        stream
                            .shutdown()
                            .await
                            .map_err(|_| MDSFTPError::ConnectionError)?;
                        return Err(MDSFTPError::ConnectionError);
                    }

                    let token = String::from_utf8_lossy(&auth_header[0..64]).to_string();
                    let microservice_id = Uuid::from_bytes(
                        Bytes::try_from(&auth_header[64..80]).unwrap_or(ZERO_UUID),
                    );

                    let validation_response = req_ctx
                        .validate_peer_token(token, microservice_id)
                        .await
                        .map_err(|_| MDSFTPError::ConnectionError);

                    if validation_response.is_err() || !validation_response.unwrap().valid {
                        stream
                            .shutdown()
                            .await
                            .map_err(|_| MDSFTPError::ConnectionError)?;
                        return Err(MDSFTPError::ConnectionError);
                    }

                    debug!("Ok. Adding a new remote connection");
                    pool._internal_pool.lock().await
                        .add_connection(microservice_id, stream)
                        .await?;

                    Ok(())
                }
                .await;
            }
        });

        Ok(())
    }

    async fn create_pool(&mut self, incoming_handler: Box<dyn PacketHandler>) {
        let mut pool = MDSFTPPool::new(self.req_ctx.clone());
        pool.set_packet_handler(incoming_handler).await;
        self.pool = Some(pool)
    }
}

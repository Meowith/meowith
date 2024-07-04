use std::error::Error;
use std::io;
use std::sync::Arc;
use log::debug;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::{rustls, TlsAcceptor, TlsStream};
use uuid::{Bytes, Uuid};

use commons::context::microservice_request_context::MicroserviceRequestContext;

use crate::file_transfer::error::MDSFTPError;
use crate::file_transfer::pool::MDSFTPPool;

const ZERO_UUID: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

pub struct MDSFTPServer {
    pool: Option<MDSFTPPool>,
    req_ctx: Arc<MicroserviceRequestContext>,
}

#[allow(unused)]
impl MDSFTPServer {
    pub fn new(req_ctx: Arc<MicroserviceRequestContext>) -> Self {
        MDSFTPServer {
            pool: None,
            req_ctx,
        }
    }

    async fn start(&self, cert: &X509, key: &PKey<Private>) -> Result<(), Box<dyn Error>> {
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(cert.to_der().unwrap())],
                PrivateKeyDer::try_from(key.private_key_to_der().unwrap()).unwrap(),
            )
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener = TcpListener::bind("0.0.0.0:80").await?; // todo config
        let req_ctx = self.req_ctx.clone();

        let pool = self.pool.clone().ok_or(MDSFTPError::NoPool)?;

        tokio::spawn(async move {
            loop {
                let res: Result<(), MDSFTPError> = async {
                    let (stream, _) = listener.accept().await
                        .map_err(|_| MDSFTPError::ConnectionError)?;
                    let acceptor = acceptor.clone();
                    let stream = acceptor
                        .accept(stream)
                        .await
                        .map_err(|_| MDSFTPError::ConnectionError)?;

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
                    let microservice_id =
                        Uuid::from_bytes(Bytes::try_from(&auth_header[64..80]).unwrap_or(ZERO_UUID));

                    let validation_response = req_ctx.validate_peer_token(token, microservice_id).await
                        .map_err(|_| MDSFTPError::ConnectionError);

                    if validation_response.is_err() || !validation_response.unwrap().valid {
                        stream
                            .shutdown()
                            .await
                            .map_err(|_| MDSFTPError::ConnectionError)?;
                        return Err(MDSFTPError::ConnectionError);
                    }

                    debug!("Ok. Adding a new remote connection");
                    pool._internal_pool.add_connection(microservice_id, stream).await?;

                    Ok(())
                    // TODO prevent simulations both side conn open
                }.await;

                if res.is_err() {
                    break;
                }
            }
        });

        Ok(())
    }

    fn create_pool() -> MDSFTPPool {
        todo!()
    }
}

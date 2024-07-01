use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;

use openssl::ssl::{SslConnector, SslMethod, SslStream};
use openssl::x509::X509;

use crate::protocol::file_transfer::channel::MDSFTPChannel;
use crate::protocol::file_transfer::error::{MDSFTPError, MDSFTPResult};

#[derive(Clone)]
pub struct MDSFTPConnection {
    _internal_connection: Arc<InternalMDSFTPConnection>,
}

impl MDSFTPConnection {
    pub fn new(node_address: SocketAddr, certificate: &X509) -> MDSFTPResult<Self> {
        Ok(MDSFTPConnection {
            _internal_connection: Arc::new(InternalMDSFTPConnection::new(
                node_address,
                certificate,
            )?),
        })
    }

    pub async fn create_channel(&self) -> MDSFTPResult<MDSFTPChannel> {
        self._internal_connection.create_channel().await
    }

    pub fn close(&self) {
        todo!()
    }
}

#[allow(unused)]
struct InternalMDSFTPConnection {
    stream: SslStream<TcpStream>,
}

impl InternalMDSFTPConnection {
    fn new(addr: SocketAddr, certificate: &X509) -> MDSFTPResult<Self> {
        let mut connector_builder = SslConnector::builder(SslMethod::tls())?;

        connector_builder.set_certificate(certificate)?;

        let connector = connector_builder.build();

        let stream = TcpStream::connect(addr).map_err(|_| MDSFTPError::ConnectionError)?;
        let stream = connector
            .connect(addr.ip().to_string().as_str(), stream)
            .map_err(|_| MDSFTPError::ConnectionError)?;

        Ok(Self { stream })
    }

    pub(crate) async fn create_channel(&self) -> MDSFTPResult<MDSFTPChannel> {
        todo!()
    }
}

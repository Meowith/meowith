use std::net::SocketAddr;
use std::sync::Arc;

use openssl::ssl::{Ssl, SslContext, SslMethod};
use openssl::x509::X509;
use rand::{Rng, thread_rng};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_openssl::SslStream;

use crate::protocol::file_transfer::channel::{InternalMDSFTPChannel, MDSFTPChannel};
use crate::protocol::file_transfer::error::{MDSFTPError, MDSFTPResult};
use crate::protocol::file_transfer::net::packet_reader::PacketReader;
use crate::protocol::file_transfer::net::packet_writer::PacketWriter;

#[derive(Clone)]
pub struct MDSFTPConnection {
    _internal_connection: Arc<InternalMDSFTPConnection>,
}

impl MDSFTPConnection {
    pub async fn new(node_address: SocketAddr, certificate: &X509) -> MDSFTPResult<Self> {
        Ok(MDSFTPConnection {
            _internal_connection: Arc::new(
                InternalMDSFTPConnection::new(node_address, certificate).await?,
            ),
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
    writer: Arc<Mutex<PacketWriter>>,
    reader: Arc<PacketReader>,
    local: bool
}

impl InternalMDSFTPConnection {
    async fn new(addr: SocketAddr, certificate: &X509) -> MDSFTPResult<Self> {
        let mut ctx = SslContext::builder(SslMethod::tls())?;

        ctx.set_certificate(certificate)?;

        let stream = TcpStream::connect(addr)
            .await
            .map_err(|_| MDSFTPError::ConnectionError)?;

        let stream = SslStream::new(Ssl::new(&ctx.build()).map_err(|_| MDSFTPError::SSLError)?, stream)
            .map_err(|_| MDSFTPError::SSLError)?;


        // Note: no idea whether u can avoid using a mutex here (it is used internally),
        // preventing simultaneous access to reads and writes
        // (Or, so I think, the async r/w might help).
        // Note: U can on the raw tcp stream.
        // Note: In any case, actix uses the tokio runtime anyway.
        let split = tokio::io::split(stream);

        Ok(Self {
            writer: Arc::new(Mutex::new(PacketWriter::new(split.1))),
            reader: Arc::new(PacketReader::new(Arc::new(Mutex::new(split.0)))),
            local: true
        })
    }

    async fn generate_id(&self) -> MDSFTPResult<u32> {
        let mut rng = thread_rng();
        let map = self.reader.conn_map.read().await;
        let max_tries = 5;
        let max_ids = 1_000_000usize;

        if map.len() >= max_ids {
            return Err(MDSFTPError::MaxChannels)
        }

        let remote_offset = if self.local { 0u32 } else { 0x80000000u32 };
        for _i in 0 .. max_tries {
            let id = rng.gen_range(0 .. 0x80000000u32 ) + remote_offset;
            if !map.contains_key(&id) {
                return Ok(id)
            }
        }

        Err(MDSFTPError::MaxChannels)
    }

    pub(crate) async fn create_channel(&self) -> MDSFTPResult<MDSFTPChannel> {

        let id = self.generate_id().await?;

        let internal_ref = Arc::new(InternalMDSFTPChannel::new(
            id,
            Arc::downgrade(&self.writer),
            Arc::downgrade(&self.reader),
        ));

        self.reader.add_channel(internal_ref.clone()).await;

        Ok(MDSFTPChannel {
            _internal_channel: internal_ref
        })
    }
}

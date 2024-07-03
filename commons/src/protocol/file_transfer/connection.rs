use std::net::SocketAddr;
use std::sync::Arc;

use crate::protocol::file_transfer::channel::{InternalMDSFTPChannel, MDSFTPChannel};
use crate::protocol::file_transfer::error::{MDSFTPError, MDSFTPResult};
use crate::protocol::file_transfer::net::packet_reader::{GlobalHandler, PacketReader};
use crate::protocol::file_transfer::net::packet_type::MDSFTPPacketType;
use crate::protocol::file_transfer::net::packet_writer::PacketWriter;
use crate::protocol::file_transfer::net::wire::MDSFTPRawPacket;
use openssl::ssl::{Ssl, SslContext, SslMethod};
use openssl::x509::X509;
use rand::{thread_rng, Rng};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_openssl::SslStream;
use uuid::Uuid;

#[derive(Clone)]
pub struct MDSFTPConnection {
    _internal_connection: Arc<InternalMDSFTPConnection>,
}

impl MDSFTPConnection {
    pub async fn new(
        node_address: SocketAddr,
        certificate: &X509,
        id: Uuid,
        auth_token: &String,
        handler: GlobalHandler,
    ) -> MDSFTPResult<Self> {
        Ok(MDSFTPConnection {
            _internal_connection: Arc::new(
                InternalMDSFTPConnection::new(node_address, certificate, id, auth_token, handler).await?,
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
    channel_factory: Arc<ChannelFactory>,
    id: Uuid,
    local: bool,
}

impl InternalMDSFTPConnection {
    async fn new(
        addr: SocketAddr,
        certificate: &X509,
        id: Uuid,
        auth_token: &String,
        handler: GlobalHandler,
    ) -> MDSFTPResult<Self> {
        let mut ctx = SslContext::builder(SslMethod::tls())?;
        ctx.set_certificate(certificate)?;

        let stream = TcpStream::connect(addr)
            .await
            .map_err(|_| MDSFTPError::ConnectionError)?;

        let stream = SslStream::new(
            Ssl::new(&ctx.build()).map_err(|_| MDSFTPError::SSLError)?,
            stream,
        )
            .map_err(|_| MDSFTPError::SSLError)?;

        // Note: no idea whether u can avoid using a mutex here (it is used internally),
        // preventing simultaneous access to reads and writes
        // (Or, so I think, the async r/w might help).
        // Note: U can on the raw tcp stream.
        // Note: In any case, actix uses the tokio runtime anyway.
        let split = tokio::io::split(stream);

        let writer = Arc::new(Mutex::new(PacketWriter::new(split.1)));
        let reader = Arc::new(PacketReader::new(
            Arc::new(Mutex::new(split.0)),
            handler,
            id,
        ));
        let channel_factory = Arc::new(ChannelFactory {
            writer: writer.clone(),
            reader: reader.clone(),
        });

        reader.start(Arc::downgrade(&channel_factory));

        writer.lock().await.write_bytes(auth_token.as_bytes()).await.map_err(|_| MDSFTPError::ConnectionError)?;
        writer.lock().await.write_bytes(id.as_bytes()).await.map_err(|_| MDSFTPError::ConnectionError)?;

        Ok(Self {
            writer,
            reader,
            channel_factory,
            id,
            local: true,
        })
    }

    async fn generate_id(&self) -> MDSFTPResult<u32> {
        let mut rng = thread_rng();
        let map = self.reader.conn_map.read().await;
        let max_tries = 5;
        let max_ids = 1_000_000usize;

        if map.len() >= max_ids {
            return Err(MDSFTPError::MaxChannels);
        }

        let remote_offset = if self.local { 0u32 } else { 0x80000000u32 };
        for _i in 0..max_tries {
            let id = rng.gen_range(1..0x80000000u32) + remote_offset;
            if !map.contains_key(&id) {
                return Ok(id);
            }
        }

        Err(MDSFTPError::MaxChannels)
    }

    pub(crate) async fn create_channel(&self) -> MDSFTPResult<MDSFTPChannel> {
        let id = self.generate_id().await?;
        self.channel_factory.materialize_channel(id, true).await
    }
}

pub(crate) struct ChannelFactory {
    writer: Arc<Mutex<PacketWriter>>,
    reader: Arc<PacketReader>,
}

impl ChannelFactory {
    pub(crate) async fn materialize_channel(&self, id: u32, alert: bool) -> MDSFTPResult<MDSFTPChannel> {
        let internal_ref = Arc::new(Mutex::new(InternalMDSFTPChannel::new(
            id,
            Arc::downgrade(&self.writer),
            Arc::downgrade(&self.reader),
        )));

        self.reader.add_channel(id, internal_ref.clone()).await;

        if alert {
            self.writer
                .lock()
                .await
                .write_raw_packet(MDSFTPRawPacket {
                    packet_type: MDSFTPPacketType::ChannelOpen,
                    stream_id: id,
                    payload: vec![],
                })
                .await
                .map_err(|_| MDSFTPError::ConnectionError)?;
        }


        Ok(MDSFTPChannel {
            _internal_channel: internal_ref,
        })
    }
}

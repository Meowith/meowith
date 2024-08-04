use std::cmp::max;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rand::prelude::StdRng;
use rand::{Rng, SeedableRng};
use rustls::pki_types::{CertificateDer, IpAddr, ServerName};
use rustls::{ClientConfig, RootCertStore};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tokio_rustls::{TlsConnector, TlsStream};
use uuid::Uuid;

use crate::mdsftp::authenticator::ConnectionAuthContext;
use crate::mdsftp::channel::{InternalMDSFTPChannel, MDSFTPChannel};
use crate::mdsftp::error::{MDSFTPError, MDSFTPResult};
use crate::mdsftp::net::packet_reader::{GlobalHandler, PacketReader};
use crate::mdsftp::net::packet_type::MDSFTPPacketType;
use crate::mdsftp::net::packet_writer::PacketWriter;
use crate::mdsftp::net::wire::MDSFTPRawPacket;

#[derive(Clone)]
pub struct MDSFTPConnection {
    _internal_connection: Arc<InternalMDSFTPConnection>,
}

impl MDSFTPConnection {
    pub async fn new(
        node_address: SocketAddr,
        connection_auth_context: &ConnectionAuthContext,
        id: Uuid,
        handler: GlobalHandler,
    ) -> MDSFTPResult<Self> {
        let conn = Self::create_conn(connection_auth_context, &node_address).await?;

        Ok(MDSFTPConnection {
            _internal_connection: Arc::new(
                InternalMDSFTPConnection::new(id, handler, conn, true).await?,
            ),
        })
    }

    pub async fn from_conn(
        id: Uuid,
        handler: GlobalHandler,
        conn: TlsStream<TcpStream>,
    ) -> MDSFTPResult<Self> {
        Ok(MDSFTPConnection {
            _internal_connection: Arc::new(
                InternalMDSFTPConnection::new(id, handler, conn, false).await?,
            ),
        })
    }

    async fn create_conn(
        connection_auth_context: &ConnectionAuthContext,
        addr: &SocketAddr,
    ) -> MDSFTPResult<TlsStream<TcpStream>> {
        let mut root_cert_store = RootCertStore::empty();
        root_cert_store
            .add(CertificateDer::from(
                connection_auth_context
                    .root_certificate
                    .to_der()
                    .map_err(MDSFTPError::from)?,
            ))
            .map_err(MDSFTPError::from)?;
        let config = ClientConfig::builder()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let server_name = ServerName::IpAddress(IpAddr::from(addr.ip()));

        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|_| MDSFTPError::ConnectionError)?;
        let mut stream = TlsStream::from(
            connector
                .connect(server_name, stream)
                .await
                .map_err(MDSFTPError::from)?,
        );

        stream
            .write_all(connection_auth_context.own_id.as_bytes())
            .await
            .map_err(|_| MDSFTPError::ConnectionError)?;

        if let Some(auth) = &connection_auth_context.authenticator {
            auth.authenticate_outgoing(&mut stream).await?;
        }

        Ok(stream)
    }

    pub fn local_id(&self) -> Uuid {
        self._internal_connection.local_id
    }

    pub async fn create_channel(&self) -> MDSFTPResult<MDSFTPChannel> {
        self._internal_connection.create_channel().await
    }

    pub fn channel_count(&self) -> usize {
        self._internal_connection.channel_count()
    }

    pub fn safe_to_close(&self) -> bool {
        self.channel_count() == 0
    }

    pub async fn last_read(&self) -> Instant {
        self._internal_connection.last_read().await
    }

    pub async fn last_write(&self) -> Instant {
        self._internal_connection.last_write().await
    }

    pub async fn last_access(&self) -> Instant {
        let last_read = self.last_read().await;
        let last_write = self.last_read().await;

        max(last_read, last_write)
    }

    pub async fn close(&self) {
        self._internal_connection.close().await
    }
}

#[allow(unused)]
struct InternalMDSFTPConnection {
    writer: Arc<Mutex<PacketWriter>>,
    reader: Arc<PacketReader>,
    channel_factory: Arc<ChannelFactory>,
    node_id: Uuid,
    local: bool,
    is_closing: AtomicBool,
    pub(crate) local_id: Uuid,
}

impl InternalMDSFTPConnection {
    async fn new(
        id: Uuid,
        handler: GlobalHandler,
        stream: TlsStream<TcpStream>,
        local: bool,
    ) -> MDSFTPResult<Self> {
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

        Ok(Self {
            writer,
            reader,
            channel_factory,
            node_id: id,
            local_id: Uuid::new_v4(),
            local,
            is_closing: AtomicBool::new(false),
        })
    }

    async fn generate_id(&self) -> MDSFTPResult<u32> {
        let mut rng = StdRng::from_entropy();
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

    pub(crate) fn channel_count(&self) -> usize {
        self.reader.channel_count()
    }

    pub(crate) async fn create_channel(&self) -> MDSFTPResult<MDSFTPChannel> {
        if !self.reader.running.load(Ordering::Relaxed) {
            return Err(MDSFTPError::ConnectionError);
        }
        if self.is_closing.load(Ordering::Relaxed) {
            return Err(MDSFTPError::Interrupted);
        }
        let id = self.generate_id().await?;
        self.channel_factory.materialize_channel(id, true).await
    }

    pub(super) async fn close(&self) {
        let _ = &self.is_closing.store(true, Ordering::SeqCst);
        self.reader.close().await;
        let mut writer = self.writer.lock().await;
        writer.close();
        writer.stream.shutdown().await.expect("Shutdown failed");
    }

    pub(crate) async fn last_read(&self) -> Instant {
        self.reader.last_read().await
    }

    pub(crate) async fn last_write(&self) -> Instant {
        self.writer.lock().await.last_write().await
    }
}

pub(crate) struct ChannelFactory {
    writer: Arc<Mutex<PacketWriter>>,
    reader: Arc<PacketReader>,
}

/// The `ChannelFactory` is Responsible for actually creating and configuring MDSFTP channels.
impl ChannelFactory {
    pub(crate) async fn materialize_channel(
        &self,
        id: u32,
        alert: bool,
    ) -> MDSFTPResult<MDSFTPChannel> {
        let internal_ref = Arc::new(InternalMDSFTPChannel::new(
            id,
            Arc::downgrade(&self.writer),
            Arc::downgrade(&self.reader),
        ));

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

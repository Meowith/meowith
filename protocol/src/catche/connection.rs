use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::TlsStream;
use uuid::Uuid;
use crate::catche::error::CatcheError;
use crate::catche::reader::{CatchePacketHandler, PacketReader};
use crate::catche::writer::PacketWriter;

#[derive(Clone)]
pub struct CatcheConnection {
    _internal_connection: Arc<InternalCatcheConnection>,
}

impl CatcheConnection {
    pub async fn from_conn(conn: TlsStream<TcpStream>, callback: CatchePacketHandler) -> Result<Self, CatcheError>  {
        Ok(CatcheConnection {
            _internal_connection: Arc::new(InternalCatcheConnection::new(conn, callback).await?)
        })
    }

    pub async fn write_invalidate_packet(&self) -> std::io::Result<()> {
        self._internal_connection.writer.lock().await.write_invalidate_packet().await
    }
    pub async fn write_auth_header(&self, uuid: Uuid) -> std::io::Result<()> {
        self._internal_connection.writer.lock().await.write(uuid.as_bytes()).await
    }
}

#[allow(unused)]
struct InternalCatcheConnection {
    writer: Arc<Mutex<PacketWriter>>,
    reader: Arc<PacketReader>,
    is_closing: AtomicBool,
}

impl InternalCatcheConnection {
    pub async fn new(conn: TlsStream<TcpStream>, callback: CatchePacketHandler) -> Result<Self, CatcheError> {
        let split = tokio::io::split(conn);

        let writer = Arc::new(Mutex::new(PacketWriter::new(split.1)));
        let reader = Arc::new(PacketReader::new(
            Arc::new(Mutex::new(split.0)),
            callback
        ));

        reader.start();

        Ok(Self {
            writer,
            reader,
            is_closing: AtomicBool::new(false),
        })
    }
}
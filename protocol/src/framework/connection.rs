use crate::catche::error::CatcheError;
use crate::framework::reader::{PacketParser, PacketReader};
use crate::framework::writer::PacketWriter;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;

/// Represents a single connection

#[derive(Debug)]
pub struct ProtocolConnection {
    writer: Arc<Mutex<PacketWriter>>,
    reader: Arc<PacketReader>,
    is_closing: AtomicBool,
}

impl ProtocolConnection {
    pub async fn new(
        conn: TlsStream<TcpStream>,
        packet_parser: Arc<dyn PacketParser>,
    ) -> Result<Self, CatcheError> {
        let split = tokio::io::split(conn);

        let writer = Arc::new(Mutex::new(PacketWriter::new(split.1)));
        let reader = Arc::new(PacketReader::new(split.0, packet_parser));

        reader.start();

        Ok(Self {
            writer,
            reader,
            is_closing: AtomicBool::new(false),
        })
    }

    pub async fn shutdown(&self) {
        self.is_closing.store(true, Ordering::SeqCst);
        self.writer.lock().unwrap().close();
        self.reader.close();
    }
}

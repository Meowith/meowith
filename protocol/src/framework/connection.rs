use crate::catche::error::CatcheError;
use crate::framework::packet::parser::{PacketParser, PacketBuilder, Packet};
use crate::framework::writer::PacketWriter;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio_rustls::TlsStream;
use crate::framework::reader::PacketReader;

/// Represents a single connection

#[derive(Debug)]
pub struct ProtocolConnection {
    writer: Arc<Mutex<PacketWriter>>,
    reader: Arc<PacketReader>,
    is_closing: AtomicBool,
}

impl <T: Packet + 'static + Send> ProtocolConnection<T> {
    pub async fn new(
        conn: TlsStream<TcpStream>,
        packet_parser: Arc<dyn PacketParser<T>>,
        packet_builder: Arc<dyn PacketBuilder<T>>,
    ) -> Result<Self, CatcheError> {
        let split = tokio::io::split(conn);

        let writer = Arc::new(Mutex::new(PacketWriter::new(split.1, packet_builder)));
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

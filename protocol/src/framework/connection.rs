use crate::framework::error::{ProtocolError, ProtocolResult};
use crate::framework::reader::PacketReader;
use crate::framework::traits::{Packet, PacketDispatcher, PacketSerializer};
use crate::framework::writer::PacketWriter;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::TlsStream;

/// Represents a single connection

#[derive(Clone)]
pub struct ProtocolConnection<T: Packet + 'static + Send>(Arc<InternalProtocolConnection<T>>);

#[derive(Debug)]
struct InternalProtocolConnection<T: Packet + 'static + Send> {
    writer: Arc<Mutex<PacketWriter<T>>>,
    reader: PacketReader<T>,
    is_closing: AtomicBool,
}

impl<T: Packet + 'static + Send> InternalProtocolConnection<T> {
    pub async fn new(
        conn: TlsStream<TcpStream>,
        dispatcher: Arc<dyn PacketDispatcher<T>>,
        serializer: Arc<dyn PacketSerializer<T>>,
    ) -> Self {
        let split = tokio::io::split(conn);

        let writer = Arc::new(Mutex::new(PacketWriter::new(split.1, serializer)));
        let reader = PacketReader::new(split.0, dispatcher);

        reader.start();

        Self {
            writer,
            reader,
            is_closing: AtomicBool::new(false),
        }
    }

    pub fn obtain_writer(&self) -> Arc<Mutex<PacketWriter<T>>> {
        Arc::clone(&self.writer)
    }

    pub async fn shutdown(&self) -> ProtocolResult<()> {
        self.is_closing.store(true, Ordering::SeqCst);
        self.reader.close();
        self.writer
            .lock()
            .await
            .close()
            .await
            .map_err(|_| ProtocolError::ShuttingDown)?;

        Ok(())
    }
}

impl<T: Packet + 'static + Send> Drop for ProtocolConnection<T> {
    fn drop(&mut self) {
        let internal = self.0.clone();
        tokio::spawn(async move {
            if let Err(e) = internal.shutdown().await {
                log::error!("Error while shutting down protocol connection {:?}", e);
            }
        });
    }
}

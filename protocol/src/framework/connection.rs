use crate::framework::error::{ProtocolError, ProtocolResult};
use crate::framework::reader::PacketReader;
use crate::framework::traits::{Packet, PacketDispatcher};
use crate::framework::writer::PacketWriter;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::ReadHalf;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::TlsStream;

/// Represents a single connection

#[derive(Clone, Debug)]
pub struct ProtocolConnection<T: Packet + 'static + Send>(pub Arc<InternalProtocolConnection<T>>);

impl<T: Packet + 'static + Send> ProtocolConnection<T> {
    pub fn new(
        reader_stream: ReadHalf<TlsStream<TcpStream>>,
        dispatcher: Arc<dyn PacketDispatcher<T>>,
        writer: Arc<Mutex<PacketWriter<T>>>,
    ) -> Self {
        Self(Arc::new(InternalProtocolConnection::new(
            reader_stream,
            dispatcher,
            writer,
        )))
    }
}

#[derive(Debug)]
pub struct InternalProtocolConnection<T: Packet + 'static + Send> {
    writer: Arc<Mutex<PacketWriter<T>>>,
    reader: PacketReader<T>,
    is_closing: AtomicBool,
}

impl<T: Packet + 'static + Send> InternalProtocolConnection<T> {
    fn new(
        reader_stream: ReadHalf<TlsStream<TcpStream>>,
        dispatcher: Arc<dyn PacketDispatcher<T>>,
        writer: Arc<Mutex<PacketWriter<T>>>,
    ) -> Self {
        let reader = PacketReader::new(reader_stream, dispatcher);

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

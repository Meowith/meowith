use crate::framework::reader::PacketReader;
use crate::framework::traits::{Packet, PacketDispatcher};
use crate::framework::writer::PacketWriter;
use commons::error::protocol_error::{ProtocolError, ProtocolResult};
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use log::trace;
use tokio::io::ReadHalf;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{channel, Receiver};
use tokio::sync::Mutex;
use tokio_rustls::TlsStream;

/// Represents a single connection

#[derive(Clone, Debug)]
pub struct ProtocolConnection<T: Packet + 'static + Send>(pub Arc<InternalProtocolConnection<T>>);

impl<T: Packet + 'static + Send> Deref for ProtocolConnection<T> {
    type Target = InternalProtocolConnection<T>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

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
    #[allow(unused)]
    pub shutdown_receiver: Arc<Mutex<Receiver<()>>>,
}

impl<T: Packet + 'static + Send> InternalProtocolConnection<T> {
    fn new(
        reader_stream: ReadHalf<TlsStream<TcpStream>>,
        dispatcher: Arc<dyn PacketDispatcher<T>>,
        writer: Arc<Mutex<PacketWriter<T>>>,
    ) -> Self {
        let (sender, receiver) = channel(1);

        let mut reader = PacketReader::new(reader_stream, dispatcher);
        reader.shutdown_notify = Some(sender);
        reader.start();

        Self {
            writer,
            reader,
            is_closing: AtomicBool::new(false),
            shutdown_receiver: Arc::new(Mutex::new(receiver)),
        }
    }

    pub async fn write_packet(&self, packet: T) -> ProtocolResult<()> {
        trace!("InternalConnection::write_packet({:?})", packet);
        match self.writer.lock().await.write_packet(packet).await {
            Ok(_) => Ok(()),
            Err(ProtocolError::WriteError(_)) => self.shutdown(true).await,
            Err(e) => Err(e),
        }
    }

    pub async fn write(&self, payload: &[u8]) -> ProtocolResult<()> {
        match self.writer.lock().await.write(payload).await {
            Ok(_) => Ok(()),
            Err(_) => self.shutdown(true).await,
        }
    }

    /// `notify` will notify the keeper task.
    /// Should be `false` when attempting to close the connection for good.
    pub async fn shutdown(&self, notify: bool) -> ProtocolResult<()> {
        self.is_closing.store(true, Ordering::SeqCst);
        // Note: this will call the shutdown sender
        self.reader.close(notify).await;
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
            if let Err(e) = internal.shutdown(false).await {
                log::error!("Error while shutting down protocol connection {:?}", e);
            }
        });
    }
}

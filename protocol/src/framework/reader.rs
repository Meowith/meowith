use log::error;
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::framework::error::ProtocolError;
use crate::framework::traits::{Packet, PacketDispatcher};
use tokio::io::ReadHalf;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_rustls::TlsStream;

#[derive(Debug)]
pub(crate) struct PacketReader<T: Packet + 'static + Send> {
    stream: Arc<Mutex<ReadHalf<TlsStream<TcpStream>>>>,
    running: Arc<AtomicBool>,
    dispatcher: Arc<dyn PacketDispatcher<T>>,
    last_read: Arc<Mutex<Instant>>,
}

impl<T: Packet + 'static + Send> PacketReader<T> {
    /// Creates a new PacketReader with an abstracted packet dispatcher
    pub(crate) fn new(
        stream: ReadHalf<TlsStream<TcpStream>>,
        dispatcher: Arc<dyn PacketDispatcher<T>>,
    ) -> Self {
        PacketReader {
            stream: Arc::new(Mutex::new(stream)),
            running: Arc::new(AtomicBool::new(false)),
            dispatcher,
            last_read: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Starts reading packets from the stream
    pub(crate) fn start(&self) -> JoinHandle<()> {
        let stream = self.stream.clone();
        let running = self.running.clone();
        let parser = self.dispatcher.clone();
        let last_read = self.last_read.clone();

        running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            let mut stream = stream.lock().await;
            while running.load(Ordering::Relaxed) {
                match parser.dispatch_packet(&mut stream).await {
                    Ok(_) => {
                        *last_read.lock().await = Instant::now();
                    }
                    Err(ProtocolError::ReadError(err)) => {
                        error!("Stream read error: {}", err);
                        break;
                    }
                    Err(err) => {
                        error!("Packet parse error: {}", err);
                        break;
                    }
                }
            }
        })
    }

    /// Stops the packet reader gracefully
    pub(crate) fn close(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

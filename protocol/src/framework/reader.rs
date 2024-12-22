use log::error;
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::framework::packet::parser::{Packet, PacketParser};
use tokio::io::ReadHalf;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_rustls::TlsStream;

/// Custom error type for packet parsing issues
#[derive(Debug, thiserror::Error)]
pub enum PacketParseError {
    #[error("Failed to read packet from stream")]
    ReadError(#[from] tokio::io::Error),
    #[error("Invalid packet format")]
    InvalidFormat,
    #[error("Unexpected packet size")]
    SizeMismatch,
}

#[derive(Debug)]
pub(crate) struct PacketReader<T: Packet + 'static + Send> {
    stream: Arc<Mutex<ReadHalf<TlsStream<TcpStream>>>>,
    running: Arc<AtomicBool>,
    parser: Arc<dyn PacketParser<T>>,
    last_read: Arc<Mutex<Instant>>,
}

impl<T: Packet + 'static + Send> PacketReader<T> {
    /// Creates a new PacketReader with an abstracted packet parser
    pub(crate) fn new(
        stream: ReadHalf<TlsStream<TcpStream>>,
        parser: Arc<dyn PacketParser<T>>,
    ) -> Self {
        PacketReader {
            stream: Arc::new(Mutex::new(stream)),
            running: Arc::new(AtomicBool::new(false)),
            parser,
            last_read: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Starts reading packets from the stream
    pub(crate) fn start(&self) -> JoinHandle<()> {
        let stream = self.stream.clone();
        let running = self.running.clone();
        let parser = self.parser.clone();
        let last_read = self.last_read.clone();

        running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            let mut stream = stream.lock().await;
            while running.load(Ordering::Relaxed) {
                match parser.parse_packet(&mut stream).await {
                    Ok(_) => {
                        *last_read.lock().await = Instant::now();
                    }
                    Err(PacketParseError::ReadError(err)) => {
                        error!("Stream read error: {}", err);
                        break;
                    }
                    Err(err) => {
                        error!("Packet parse error: {}", err);
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

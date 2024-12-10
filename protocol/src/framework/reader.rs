use async_trait::async_trait;
use log::{error, trace};
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::io::ReadHalf;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_rustls::server::TlsStream;

/// Trait for parsing incoming packets from the stream
#[async_trait]
pub trait PacketParser: Send + Debug + Sync + 'static {
    /// Parses a packet from the given stream. Returns the parsed data or an error.
    async fn parse_packet(
        &self,
        stream: &mut ReadHalf<TlsStream<TcpStream>>,
    ) -> Result<(), PacketParseError>;
}

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
pub(crate) struct PacketReader {
    stream: Arc<Mutex<ReadHalf<TlsStream<TcpStream>>>>,
    running: Arc<AtomicBool>,
    parser: Arc<dyn PacketParser>,
    last_read: Arc<Mutex<Instant>>,
}

impl PacketReader {
    /// Creates a new PacketReader with an abstracted packet parser
    pub(crate) fn new(
        stream: ReadHalf<TlsStream<TcpStream>>,
        parser: Arc<dyn PacketParser>,
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
                        // Update last read timestamp
                        *last_read.lock().await = Instant::now();
                    }
                    Err(PacketParseError::ReadError(err)) => {
                        error!("Stream read error: {}", err);
                        break; // Connection is likely closed
                    }
                    Err(err) => {
                        error!("Packet parse error: {}", err);
                        // Decide if you want to continue on a parse error
                    }
                }
            }

            trace!("Reader loop closed");
        })
    }

    /// Stops the packet reader gracefully
    pub(crate) fn close(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

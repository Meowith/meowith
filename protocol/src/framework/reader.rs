use log::error;
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::framework::traits::{Packet, PacketDispatcher};
use commons::error::protocol_error::ProtocolError;
use tokio::io::ReadHalf;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_rustls::TlsStream;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub(crate) struct PacketReader<T: Packet + 'static + Send> {
    stream: Arc<Mutex<ReadHalf<TlsStream<TcpStream>>>>,
    // Note:
    // shared with the connection instance
    // to also immediately prevent write-ops after closure.
    shutting_down: Arc<AtomicBool>,
    dispatcher: Arc<dyn PacketDispatcher<T>>,
    last_read: Arc<Mutex<Instant>>,
    read_cancellation_token: CancellationToken,
    pub(crate) shutdown_notify: Option<Sender<()>>,
}

impl<T: Packet + 'static + Send> PacketReader<T> {
    /// Creates a new PacketReader with an abstracted packet dispatcher
    pub(crate) fn new(
        stream: ReadHalf<TlsStream<TcpStream>>,
        dispatcher: Arc<dyn PacketDispatcher<T>>,
        shutting_down: Arc<AtomicBool>,
    ) -> Self {
        PacketReader {
            stream: Arc::new(Mutex::new(stream)),
            shutting_down,
            dispatcher,
            last_read: Arc::new(Mutex::new(Instant::now())),
            read_cancellation_token: CancellationToken::new(),
            shutdown_notify: None,
        }
    }

    /// Starts reading packets from the stream
    pub(crate) fn start(&self) -> JoinHandle<()> {
        let stream = self.stream.clone();
        let shutting_down = self.shutting_down.clone();
        let parser = self.dispatcher.clone();
        let last_read = self.last_read.clone();
        let read_cancellation_token = self.read_cancellation_token.clone();
        let shutdown_notify = self.shutdown_notify.clone();

        tokio::spawn(async move {
            let mut stream = stream.lock().await;
            while !shutting_down.load(Ordering::Relaxed) {
                match parser
                    .parse_and_dispatch_packet(&mut stream, &read_cancellation_token)
                    .await
                {
                    Ok(_) => {
                        *last_read.lock().await = Instant::now();
                    }
                    Err(ProtocolError::ReadError(err)) => {
                        error!("Stream read error: {}", err);
                        if let Some(sender) = shutdown_notify.as_ref() {
                            shutting_down.store(true, Ordering::SeqCst);
                            let _ = sender.send(()).await;
                        }
                        break;
                    }
                    Err(err) => {
                        error!("Packet parse error: {}", err);
                        if let Some(sender) = shutdown_notify.as_ref() {
                            shutting_down.store(true, Ordering::SeqCst);
                            let _ = sender.send(()).await;
                        }
                        break;
                    }
                }
            }
        })
    }

    /// Stops the packet reader gracefully
    pub(crate) async fn close(&self, notify: bool) {
        if self.shutting_down.load(Ordering::Relaxed) {
            return;
        }
        self.shutting_down.store(true, Ordering::SeqCst);
        self.read_cancellation_token.cancel();
        if notify {
            if let Some(sender) = self.shutdown_notify.as_ref() {
                let _ = sender.send(()).await;
            }
        }
    }
}

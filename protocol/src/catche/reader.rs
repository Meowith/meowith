use log::debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, ReadHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_rustls::TlsStream;

use crate::catche::handler::CatcheHandler;
use crate::catche::writer::PACKET_SIZE;

pub type CatchePacketHandler = Arc<Mutex<Box<dyn CatcheHandler>>>;

pub(crate) struct PacketReader {
    pub(crate) stream: Arc<Mutex<ReadHalf<TlsStream<TcpStream>>>>,
    pub(crate) running: Arc<AtomicBool>,
    handler: CatchePacketHandler,
    last_read: Arc<Mutex<Instant>>,
}

impl PacketReader {
    pub(crate) fn new(
        stream: Arc<Mutex<ReadHalf<TlsStream<TcpStream>>>>,
        handler: CatchePacketHandler,
    ) -> Self {
        PacketReader {
            stream,
            running: Arc::new(AtomicBool::new(false)),
            handler,
            last_read: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub(crate) fn start(&self) -> JoinHandle<()> {
        let stream_ref = self.stream.clone();
        let running = self.running.clone();
        let handler = self.handler.clone();
        let last_read = self.last_read.clone();
        running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            let mut stream = stream_ref.lock().await;
            let mut packet_buf: [u8; PACKET_SIZE] = [0; PACKET_SIZE];

            while running.load(Ordering::Relaxed) {
                if stream.read_exact(&mut packet_buf).await.is_err() {
                    debug!("Packet read failed");
                    break;
                };

                if packet_buf[0] != 1 {
                    debug!("Invalid packet received");
                    break;
                }

                let _ = handler.lock().await.handle_invalidate().await;
                *last_read.lock().await = Instant::now();
            }
            debug!("Reader loop close");
        })
    }

    #[allow(unused)]
    pub(crate) fn close(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

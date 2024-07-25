use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_rustls::TlsStream;

pub const PACKET_SIZE: usize = 1;
pub const INVALIDATE_PAYLOAD: [u8; 1] = [1];

pub(crate) struct PacketWriter {
    pub(crate) stream: WriteHalf<TlsStream<TcpStream>>,
    last_write: Instant,
}

impl PacketWriter {
    pub(crate) fn new(stream: WriteHalf<TlsStream<TcpStream>>) -> Self {
        PacketWriter {
            stream,
            last_write: Instant::now(),
        }
    }

    pub(crate) async fn write_invalidate_packet(&mut self) -> std::io::Result<()> {
        let _ = self.write(INVALIDATE_PAYLOAD.as_slice()).await;
        Ok(())
    }

    pub(crate) async fn write(&mut self, slice: &[u8]) -> std::io::Result<()> {
        self.last_write = Instant::now();

        let _ = self.stream.write(slice).await?;

        Ok(())
    }

    pub(crate) async fn last_write(&self) -> Instant {
        self.last_write
    }

    pub(crate) fn close(&mut self) {}
}
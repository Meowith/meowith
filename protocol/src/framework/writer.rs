use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_rustls::server::TlsStream;

#[derive(Debug)]
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

    pub(crate) async fn write(&mut self, slice: &[u8]) -> std::io::Result<()> {
        self.last_write = Instant::now();

        self.stream.write_all(slice).await?;

        Ok(())
    }

    #[allow(unused)]
    pub(crate) async fn last_write(&self) -> Instant {
        self.last_write
    }

    pub(crate) fn close(&mut self) {}
}

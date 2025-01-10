use crate::framework::error::{ProtocolError, ProtocolResult};
use crate::framework::traits::{Packet, PacketSerializer};
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_rustls::TlsStream;

#[derive(Debug)]
pub struct PacketWriter<T: Packet + 'static + Send> {
    pub(crate) stream: WriteHalf<TlsStream<TcpStream>>,
    last_write: Instant,
    serializer: Arc<dyn PacketSerializer<T>>,
}

impl<T: Packet + 'static + Send> PacketWriter<T> {
    pub fn new(
        stream: WriteHalf<TlsStream<TcpStream>>,
        serializer: Arc<dyn PacketSerializer<T>>,
    ) -> Self {
        PacketWriter {
            stream,
            serializer,
            last_write: Instant::now(),
        }
    }

    async fn write(&mut self, slice: &[u8]) -> std::io::Result<()> {
        self.stream.write_all(slice).await?;
        self.last_write = Instant::now();

        Ok(())
    }

    pub(crate) async fn write_packet(&mut self, packet: T) -> ProtocolResult<()> {
        let packet = self.serializer.serialize_packet(packet);

        self.write(packet.as_slice())
            .await
            .map_err(ProtocolError::WriteError)?;

        Ok(())
    }

    #[allow(unused)]
    pub(crate) async fn last_write(&self) -> Instant {
        self.last_write
    }

    pub(crate) async fn close(&mut self) -> std::io::Result<()> {
        self.stream.shutdown().await?;

        Ok(())
    }
}

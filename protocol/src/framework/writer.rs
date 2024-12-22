use crate::framework::error::PacketBuildError;
use crate::framework::packet::parser::{Packet, PacketBuilder};
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_rustls::TlsStream;

#[derive(Debug)]
pub(crate) struct PacketWriter<T: Packet + 'static + Send> {
    pub(crate) stream: WriteHalf<TlsStream<TcpStream>>,
    last_write: Instant,
    builder: Arc<dyn PacketBuilder<T>>,
}

impl<T: Packet + 'static + Send> PacketWriter<T> {
    pub(crate) fn new(
        stream: WriteHalf<TlsStream<TcpStream>>,
        builder: Arc<dyn PacketBuilder<T>>,
    ) -> Self {
        PacketWriter {
            stream,
            builder,
            last_write: Instant::now(),
        }
    }

    async fn write(&mut self, slice: &[u8]) -> std::io::Result<()> {
        self.stream.write_all(slice).await?;
        self.last_write = Instant::now();

        Ok(())
    }

    pub(crate) async fn write_packet(&mut self, packet: T) -> Result<(), PacketBuildError> {
        let packet = self.builder.build_packet(packet)?;

        self.write(packet.as_slice())
            .await
            .map_err(PacketBuildError::WriteError)?;

        Ok(())
    }

    #[allow(unused)]
    pub(crate) async fn last_write(&self) -> Instant {
        self.last_write
    }

    pub(crate) fn close(&mut self) {}
}

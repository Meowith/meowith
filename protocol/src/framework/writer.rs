use crate::framework::traits::{Packet, PacketSerializer};
use commons::error::protocol_error::{ProtocolError, ProtocolResult};
use std::sync::Arc;
use log::trace;
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_rustls::TlsStream;

#[derive(Debug)]
pub struct PacketWriter<T: Packet + 'static + Send> {
    pub stream: WriteHalf<TlsStream<TcpStream>>,
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

    pub async fn write(&mut self, slice: &[u8]) -> std::io::Result<()> {
        self.stream.write_all(slice).await?;
        self.last_write = Instant::now();

        Ok(())
    }

    pub async fn write_packet(&mut self, packet: T) -> ProtocolResult<()> {
        let packet = self.serializer.serialize_packet(packet);

        trace!("Sending packet to writer: {:?}", packet);

        let res = self.write(packet.as_slice())
            .await
            .map_err(ProtocolError::WriteError);

        trace!("Sending packet to writer: {:?}", res);
        res
    }

    #[allow(unused)]
    pub(crate) async fn last_write(&self) -> Instant {
        self.last_write
    }

    pub(crate) async fn close(&mut self) -> std::io::Result<()> {
        trace!("Protocol writer closing");
        self.stream.shutdown().await?;

        Ok(())
    }
}

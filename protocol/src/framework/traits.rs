use async_trait::async_trait;
use commons::error::protocol_error::ProtocolResult;
use std::fmt::Debug;
use tokio::io::ReadHalf;
use tokio::net::TcpStream;
use tokio_rustls::TlsStream;
use tokio_util::sync::CancellationToken;

/// Trait for parsing incoming packets from the stream
#[async_trait]
pub trait PacketDispatcher<T: Packet>: Send + Debug + Sync + 'static {
    /// Parses a packet from the given stream. Calls the associated PacketHandler method.
    async fn parse_and_dispatch_packet(
        &self,
        stream: &mut ReadHalf<TlsStream<TcpStream>>,
        read_cancellation: &CancellationToken,
    ) -> ProtocolResult<()>;
}

pub trait PacketSerializer<T: Packet>: Send + Debug + Sync + 'static {
    /// Builds and serializes a packet. Returns the serialized packet data.
    fn serialize_packet(&self, packet: T) -> Vec<u8>;
}

pub trait Packet: Debug {
    fn validate_length(&self, len: u32) -> bool;
}

use crate::framework::error::PacketBuildError;
use crate::framework::reader::PacketParseError;
use async_trait::async_trait;
use std::fmt::Debug;
use tokio::io::ReadHalf;
use tokio::net::TcpStream;
use tokio_rustls::TlsStream;

/// Trait for parsing incoming packets from the stream
#[async_trait]
pub trait PacketDispatcher<T: Packet>: Send + Debug + Sync + 'static {
    /// Parses a packet from the given stream. Calls the associated PacketHandler method.
    async fn dispatch_packet(
        &self,
        stream: &mut ReadHalf<TlsStream<TcpStream>>,
    ) -> Result<(), PacketParseError>;
}

pub trait PacketSerializer<T: Packet>: Send + Debug + Sync + 'static {
    /// Builds and serializes a packet. Returns the serialized packet data or an error.
    fn serialize_packet(&self, packet: T) -> Result<Vec<u8>, PacketBuildError>;
}

pub trait Packet: Debug {
    fn validate_length(&self, len: u32) -> bool;
}

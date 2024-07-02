use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio_openssl::SslStream;

#[allow(unused)]
pub(crate) struct PacketWriter {
    stream: WriteHalf<SslStream<TcpStream>>,
}

impl PacketWriter {
    pub fn new(stream: WriteHalf<SslStream<TcpStream>>) -> Self {
        PacketWriter {
            stream,
        }
    }

    #[allow(unused)]
    pub(crate) async fn write_packet(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.stream.write_all(data).await
    }
}

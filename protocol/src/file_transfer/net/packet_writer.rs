use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio_rustls::TlsStream;

use crate::file_transfer::net::wire::{write_header, MDSFTPHeader, MDSFTPRawPacket, PAYLOAD_SIZE};

pub(crate) struct PacketWriter {
    pub(crate) stream: WriteHalf<TlsStream<TcpStream>>,
    header_buf: [u8; 7],
}

impl PacketWriter {
    pub(crate) fn new(stream: WriteHalf<TlsStream<TcpStream>>) -> Self {
        PacketWriter {
            stream,
            header_buf: [0u8; 7],
        }
    }

    pub(crate) async fn write_raw_packet(&mut self, data: MDSFTPRawPacket) -> std::io::Result<()> {
        if data.payload.len() > PAYLOAD_SIZE {
            panic!("Payload too large {}", data.payload.len());
        }

        write_header(
            &MDSFTPHeader {
                packet_id: (&data.packet_type).into(),
                stream_id: data.stream_id,
                payload_size: data.payload.len() as u16,
            },
            &mut self.header_buf,
        );

        self.stream.write_all(&self.header_buf).await?;
        self.stream.write_all(&data.payload).await
    }

    pub(crate) fn close(&mut self) {}
}

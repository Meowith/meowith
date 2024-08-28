use crate::mdsftp::net::packet_type::MDSFTPPacketType;
use crate::mdsftp::net::wire::{
    write_header, MDSFTPHeader, MDSFTPRawPacket, HEADER_SIZE, PAYLOAD_SIZE,
};
use log::trace;
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_rustls::TlsStream;

pub(crate) struct PacketWriter {
    pub(crate) stream: WriteHalf<TlsStream<TcpStream>>,
    header_buf: [u8; HEADER_SIZE],
    last_write: Instant,
}

impl PacketWriter {
    pub(crate) fn new(stream: WriteHalf<TlsStream<TcpStream>>) -> Self {
        PacketWriter {
            stream,
            header_buf: [0u8; HEADER_SIZE],
            last_write: Instant::now(),
        }
    }

    pub(crate) async fn write_raw_packet(&mut self, data: MDSFTPRawPacket) -> std::io::Result<()> {
        if data.payload.len() > PAYLOAD_SIZE {
            panic!("Payload too large {}", data.payload.len());
        }

        write_header(
            &MDSFTPHeader {
                packet_id: data.packet_type.into(),
                stream_id: data.stream_id,
                payload_size: data.payload.len() as u32,
            },
            &mut self.header_buf,
        );

        self.last_write = Instant::now();
        self.stream.write_all(&self.header_buf).await?;
        self.stream.write_all(&data.payload).await?;
        trace!(
            "Packet writer -> RAW: type={:?} len={}",
            data.packet_type,
            data.payload.len()
        );
        Ok(())
    }

    // Avoid creating a secondary payload buffer.
    pub(crate) async fn write_chunk(
        &mut self,
        stream_id: u32,
        payload_header: &[u8],
        payload: &[u8],
    ) -> std::io::Result<()> {
        let total_size = (payload_header.len() + payload.len()) as u32;
        write_header(
            &MDSFTPHeader {
                packet_id: MDSFTPPacketType::FileChunk.into(),
                stream_id,
                payload_size: total_size,
            },
            &mut self.header_buf,
        );

        self.last_write = Instant::now();
        self.stream.write_all(&self.header_buf).await?;
        self.stream.write_all(payload_header).await?;
        self.stream.write_all(payload).await?;
        trace!(
            "Packet writer -> CHUNK: type={:?} len={total_size}",
            MDSFTPPacketType::FileChunk
        );
        Ok(())
    }

    pub(crate) async fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush().await
    }

    pub(crate) async fn last_write(&self) -> Instant {
        self.last_write
    }

    pub(crate) fn close(&mut self) {}
}

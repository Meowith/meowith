use crate::protocol::file_transfer::net::packet_type::MDSFTPPacketType;
use std::io::Write;

pub(crate) const HEADER_SIZE: usize = 7usize;
pub(crate) const PAYLOAD_SIZE: usize = 65535usize;
pub(crate) const MAX_PACKET_SIZE: usize = HEADER_SIZE + PAYLOAD_SIZE;

#[derive(Eq, PartialEq, Clone, Copy)]
pub(crate) struct MDSFTPHeader {
    pub(crate) packet_id: u8,
    pub(crate) stream_id: u32,
    pub(crate) payload_size: u16,
}

pub(crate) struct MDSFTPRawPacket {
    pub(crate) packet_type: MDSFTPPacketType,
    pub(crate) stream_id: u32,
    pub(crate) payload: Vec<u8>,
}

pub(crate) fn read_header(raw: &[u8; 7]) -> MDSFTPHeader {
    MDSFTPHeader {
        packet_id: raw[0],
        stream_id: u32::from_be_bytes(raw[1..5].try_into().unwrap()),
        payload_size: u16::from_be_bytes(raw[5..7].try_into().unwrap()),
    }
}

pub(crate) fn write_header(header: &MDSFTPHeader, buf: &mut [u8]) {
    buf[0] = header.packet_id;

    buf[1] = header.stream_id as u8;
    buf[2] = (header.stream_id >> 8) as u8;
    buf[3] = (header.stream_id >> 16) as u8;
    buf[4] = (header.stream_id >> 24) as u8;

    buf[5] = header.payload_size as u8;
    buf[6] = (header.payload_size >> 8) as u8;
}

#[cfg(test)]
mod tests {
    use crate::protocol::file_transfer::net::wire::{read_header, write_header, MDSFTPHeader};

    #[test]
    fn test_header() {
        let mut buf: [u8; 7] = [0; 7];
        let header = MDSFTPHeader {
            packet_id: 10,
            stream_id: 70201376,
            payload_size: 39676,
        };

        write_header(&header, &mut buf);
        let read = read_header(&buf);
    }
}

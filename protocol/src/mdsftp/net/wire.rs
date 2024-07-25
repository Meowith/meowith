use crate::mdsftp::net::packet_type::MDSFTPPacketType;

pub(crate) const HEADER_SIZE: usize = 9usize;
pub(crate) const PAYLOAD_SIZE: usize = u32::MAX as usize;
#[allow(unused)]
pub(crate) const MAX_PACKET_SIZE: usize = HEADER_SIZE + PAYLOAD_SIZE;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub(crate) struct MDSFTPHeader {
    pub(crate) packet_id: u8,
    pub(crate) stream_id: u32,
    pub(crate) payload_size: u32,
}

pub(crate) struct MDSFTPRawPacket {
    pub(crate) packet_type: MDSFTPPacketType,
    pub(crate) stream_id: u32,
    pub(crate) payload: Vec<u8>,
}

pub(crate) fn read_header(raw: &[u8; HEADER_SIZE]) -> MDSFTPHeader {
    MDSFTPHeader {
        packet_id: raw[0],
        stream_id: u32::from_be_bytes(raw[1..5].try_into().unwrap()),
        payload_size: u32::from_be_bytes(raw[5..9].try_into().unwrap()),
    }
}

pub(crate) fn write_header(header: &MDSFTPHeader, buf: &mut [u8]) {
    buf[0] = header.packet_id;
    buf[1..5].clone_from_slice(&header.stream_id.to_be_bytes());
    buf[5..9].clone_from_slice(&header.payload_size.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use crate::mdsftp::net::wire::{read_header, write_header, MDSFTPHeader, HEADER_SIZE};

    #[test]
    fn test_header() {
        let mut buf: [u8; HEADER_SIZE] = [0; HEADER_SIZE];
        let header = MDSFTPHeader {
            packet_id: 10,
            stream_id: 70201376,
            payload_size: 396760137,
        };

        write_header(&header, &mut buf);
        let read = read_header(&buf);

        assert_eq!(read, header);
    }
}

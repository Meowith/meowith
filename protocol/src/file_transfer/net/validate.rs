use crate::file_transfer::net::packet_type::MDSFTPPacketType;
use crate::file_transfer::net::wire::MDSFTPHeader;

pub(crate) trait PreValidate {
    /// Checks if the packet is even worth parsing.
    fn pre_validate(&self, packet: &MDSFTPHeader) -> bool;
}

/// Assumes max length pre-validation as the `&MDSFTPHeader` must have been parsed.
impl PreValidate for MDSFTPPacketType {
    fn pre_validate(&self, header: &MDSFTPHeader) -> bool {
        match self {
            MDSFTPPacketType::FileChunk => header.payload_size >= self.payload_size(),
            _ => self.payload_size() == header.payload_size,
        }
    }
}

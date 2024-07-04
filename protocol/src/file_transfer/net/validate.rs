use crate::file_transfer::net::packet_type::MDSFTPPacketType;
use crate::file_transfer::net::wire::{MDSFTPHeader};

pub(crate) trait PreValidate {

    /// Checks if the packet is even worth parsing.
    fn pre_validate(&self, packet: &MDSFTPHeader) -> bool;
}

/// Assumes max length pre-validation as the `&MDSFTPHeader` must have been parsed.
impl PreValidate for MDSFTPPacketType {
    fn pre_validate(&self, header: &MDSFTPHeader) -> bool {
        match self {
            MDSFTPPacketType::FileChunk => header.payload_size >= 6,
            MDSFTPPacketType::Retrieve => header.payload_size == 16,
            MDSFTPPacketType::Put => header.payload_size == 24,
            MDSFTPPacketType::Reserve => header.payload_size == 8,
            MDSFTPPacketType::ReserveOk => header.payload_size == 16,
            MDSFTPPacketType::ReserveErr => header.payload_size == 8,
            MDSFTPPacketType::LockReq => header.payload_size == 17,
            MDSFTPPacketType::LockAcquire => header.payload_size == 17,
            MDSFTPPacketType::LockErr => header.payload_size == 17,
            MDSFTPPacketType::ChannelOpen => header.payload_size == 0,
            MDSFTPPacketType::ChannelClose => header.payload_size == 0,
            MDSFTPPacketType::ChannelErr => header.payload_size == 0,
        }
    }
}
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive, Copy, Clone)]
#[repr(u8)]
pub(crate) enum MDSFTPPacketType {
    FileChunk = 1u8,
    Retrieve = 2u8,
    Put = 3u8,
    RecvAck = 4u8,
    Reserve = 5u8,
    ReserveCancel = 6u8,
    ReserveOk = 7u8,
    ReserveErr = 8u8,
    LockReq = 9u8,
    LockAcquire = 10u8,
    LockErr = 11u8,
    ChannelOpen = 138u8,
    ChannelClose = 129u8,
    ChannelErr = 130u8,
}

impl MDSFTPPacketType {
    pub(crate) fn is_system(&self) -> bool {
        let self_u8: u8 = (*self).into();
        self_u8 >= 128u8
    }

    pub(crate) fn payload_size(&self) -> u32 {
        match self {
            MDSFTPPacketType::FileChunk => 6,
            MDSFTPPacketType::Retrieve => 18,
            MDSFTPPacketType::Put => 24,
            MDSFTPPacketType::RecvAck => 4,
            MDSFTPPacketType::Reserve => 9,
            MDSFTPPacketType::ReserveCancel => 16,
            MDSFTPPacketType::ReserveOk => 18,
            MDSFTPPacketType::ReserveErr => 8,
            MDSFTPPacketType::LockReq => 17,
            MDSFTPPacketType::LockAcquire => 17,
            MDSFTPPacketType::LockErr => 17,
            MDSFTPPacketType::ChannelOpen => 0,
            MDSFTPPacketType::ChannelClose => 0,
            MDSFTPPacketType::ChannelErr => 0,
        }
    }
}

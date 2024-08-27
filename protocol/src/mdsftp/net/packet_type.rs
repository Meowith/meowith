use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive, Copy, Clone)]
#[repr(u8)]
pub(crate) enum MDSFTPPacketType {
    FileChunk = 1u8,
    Retrieve = 2u8,
    Put = 3u8,
    RecvAck = 4u8,
    DeleteChunk = 5u8,
    Reserve = 6u8,
    ReserveCancel = 7u8,
    ReserveOk = 8u8,
    ReserveErr = 9u8,
    LockReq = 10u8,
    LockAcquire = 11u8,
    LockErr = 12u8,
    PutOk = 13u8,
    PutErr = 14u8,
    Commit = 15u8,
    Query = 16u8,
    QueryResponse = 17u8,
    ChannelOpen = 128u8,
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
            MDSFTPPacketType::Retrieve => 34,
            MDSFTPPacketType::Put => 25,
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
            MDSFTPPacketType::DeleteChunk => 16,
            MDSFTPPacketType::PutOk => 2,
            MDSFTPPacketType::PutErr => 1,
            MDSFTPPacketType::Commit => 17,
            MDSFTPPacketType::Query => 16,
            MDSFTPPacketType::QueryResponse => 9,
        }
    }
}

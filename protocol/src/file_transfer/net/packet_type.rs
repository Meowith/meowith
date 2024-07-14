#[derive(Copy, Clone, Debug)]
pub(crate) enum MDSFTPPacketType {
    FileChunk,
    Retrieve,
    Put,
    RecvAck,
    Reserve,
    ReserveOk,
    ReserveErr,
    LockReq,
    LockAcquire,
    LockErr,
    ChannelOpen,
    ChannelClose,
    ChannelErr,
}

impl MDSFTPPacketType {
    pub(crate) fn is_system(&self) -> bool {
        let self_u8: u8 = self.into();
        self_u8 >= 128u8
    }

    pub(crate) fn payload_size(&self) -> u16 {
        match self {
            MDSFTPPacketType::FileChunk => 6,
            MDSFTPPacketType::Retrieve => 18,
            MDSFTPPacketType::Put => 24,
            MDSFTPPacketType::RecvAck => 4,
            MDSFTPPacketType::Reserve => 9,
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

impl TryFrom<u8> for MDSFTPPacketType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1u8 => Ok(MDSFTPPacketType::FileChunk),
            2u8 => Ok(MDSFTPPacketType::Retrieve),
            3u8 => Ok(MDSFTPPacketType::Put),
            4u8 => Ok(MDSFTPPacketType::RecvAck),
            5u8 => Ok(MDSFTPPacketType::Reserve),
            6u8 => Ok(MDSFTPPacketType::ReserveOk),
            7u8 => Ok(MDSFTPPacketType::ReserveErr),
            8u8 => Ok(MDSFTPPacketType::LockReq),
            9u8 => Ok(MDSFTPPacketType::LockAcquire),
            10u8 => Ok(MDSFTPPacketType::LockErr),
            128u8 => Ok(MDSFTPPacketType::ChannelOpen),
            129u8 => Ok(MDSFTPPacketType::ChannelClose),
            130u8 => Ok(MDSFTPPacketType::ChannelErr),
            _ => Err(()),
        }
    }
}

impl From<&MDSFTPPacketType> for u8 {
    fn from(value: &MDSFTPPacketType) -> Self {
        match value {
            MDSFTPPacketType::FileChunk => 1u8,
            MDSFTPPacketType::Retrieve => 2u8,
            MDSFTPPacketType::Put => 3u8,
            MDSFTPPacketType::RecvAck => 4u8,
            MDSFTPPacketType::Reserve => 5u8,
            MDSFTPPacketType::ReserveOk => 6u8,
            MDSFTPPacketType::ReserveErr => 7u8,
            MDSFTPPacketType::LockReq => 8u8,
            MDSFTPPacketType::LockAcquire => 9u8,
            MDSFTPPacketType::LockErr => 10u8,
            MDSFTPPacketType::ChannelOpen => 128u8,
            MDSFTPPacketType::ChannelClose => 129u8,
            MDSFTPPacketType::ChannelErr => 130u8,
        }
    }
}

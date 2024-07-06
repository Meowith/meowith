use crate::file_transfer::error::MDSFTPError;

pub enum LockKind {
    Read,
    Write,
}

impl From<u8> for LockKind {
    fn from(value: u8) -> Self {
        match value & 0x01 {
            0u8 => LockKind::Read,
            1u8 => LockKind::Write,
            _ => unreachable!(),
        }
    }
}

impl From<LockKind> for u8 {
    fn from(value: LockKind) -> Self {
        match value {
            LockKind::Read => 0u8,
            LockKind::Write => 1u8,
        }
    }
}

pub enum ChunkErrorKind {
    Internal,
    NotFound,
}

impl From<ChunkErrorKind> for MDSFTPError {
    fn from(value: ChunkErrorKind) -> Self {
        match value {
            ChunkErrorKind::Internal => MDSFTPError::RemoteError,
            ChunkErrorKind::NotFound => MDSFTPError::NoSuchChunkId,
        }
    }
}

impl From<u8> for ChunkErrorKind {
    fn from(value: u8) -> Self {
        match value & 0x2 {
            0u8 => ChunkErrorKind::Internal,
            2u8 => ChunkErrorKind::NotFound,
            _ => ChunkErrorKind::Internal,
        }
    }
}

impl From<ChunkErrorKind> for u8 {
    fn from(value: ChunkErrorKind) -> Self {
        match value {
            ChunkErrorKind::Internal => 0u8,
            ChunkErrorKind::NotFound => 2u8,
        }
    }
}

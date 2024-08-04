use uuid::Uuid;

use crate::mdsftp::error::MDSFTPError;

#[derive(Debug, Eq, PartialEq)]
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

#[derive(Debug, Eq, PartialEq)]
pub enum ChunkErrorKind {
    NotAvailable,
    NotFound,
}

impl From<ChunkErrorKind> for MDSFTPError {
    fn from(value: ChunkErrorKind) -> Self {
        match value {
            ChunkErrorKind::NotAvailable => MDSFTPError::RemoteError,
            ChunkErrorKind::NotFound => MDSFTPError::NoSuchChunkId,
        }
    }
}

impl From<u8> for ChunkErrorKind {
    fn from(value: u8) -> Self {
        match value & 0x2 {
            0u8 => ChunkErrorKind::NotAvailable,
            2u8 => ChunkErrorKind::NotFound,
            _ => ChunkErrorKind::NotAvailable,
        }
    }
}

impl From<ChunkErrorKind> for u8 {
    fn from(value: ChunkErrorKind) -> Self {
        match value {
            ChunkErrorKind::NotAvailable => 0u8,
            ChunkErrorKind::NotFound => 2u8,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct ReserveFlags {
    /// Indicates that the connection will receive data immediately
    pub auto_start: bool,
    /// Reservation for a durable upload
    pub durable: bool,
    /// Indicates that the channel is temporary, and that the file transfer will commence later
    pub temp: bool,
    /// Prep already existing chunk for being overwritten.
    pub overwrite: bool,
}

impl From<ReserveFlags> for u8 {
    fn from(value: ReserveFlags) -> Self {
        value.auto_start as u8
            + ((value.durable as u8) << 1u8)
            + ((value.temp as u8) << 2u8)
            + ((value.overwrite as u8) << 3u8)
    }
}

impl From<u8> for ReserveFlags {
    fn from(value: u8) -> Self {
        ReserveFlags {
            auto_start: (value & 1u8) != 0,
            durable: (value & 2u8) != 0,
            temp: (value & 4u8) != 0,
            overwrite: (value & 8u8) != 0,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct PutFlags {
    // TODO
}

impl From<PutFlags> for u8 {
    fn from(_: PutFlags) -> Self {
        0
    }
}

impl From<u8> for PutFlags {
    fn from(_: u8) -> Self {
        PutFlags {}
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ReserveResult {
    pub chunk_id: Uuid,
    pub chunk_buffer: u16,
}

#[derive(Debug, Eq, PartialEq)]
pub struct LockAcquireResult {
    pub kind: LockKind,
    pub chunk_id: Uuid,
}

#[derive(Debug, Eq, PartialEq)]
pub struct PutResult {
    pub chunk_buffer: u16,
}

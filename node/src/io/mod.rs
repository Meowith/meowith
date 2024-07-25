use std::path::Path;

use heim::disk::usage;

use crate::io::error::{MeowithIoError, MeowithIoResult};

pub mod error;
pub mod fragment_ledger;

#[derive(PartialOrd, PartialEq)]
pub struct SpaceUsage {
    pub total: u64,
    pub used: u64,
    pub free: u64,
}

pub async fn get_space<P: AsRef<Path>>(path: P) -> MeowithIoResult<SpaceUsage> {
    let usage = usage(path)
        .await
        .map_err(|_| MeowithIoError::InsufficientDiskSpace)?;
    Ok(SpaceUsage {
        // The Unit is Bytes
        total: usage.total().value,
        used: usage.used().value,
        free: usage.free().value,
    })
}

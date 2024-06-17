#[derive(Debug)]
#[allow(unused)]
pub enum ConfigError {
    InternalError,
    InvalidIpAddress,
    InvalidPort,
    InsufficientDiskSpace,

    InvalidSizeNumber,
    InvalidSizeUnit
}
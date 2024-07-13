#[derive(Debug)]
pub enum ConfigError {
    InvalidIpAddress,
    InvalidPort,
    InsufficientDiskSpace,

    InvalidSizeNumber,
    InvalidSizeUnit,
}

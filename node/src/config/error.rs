#[derive(Debug)]
pub enum ConfigError {
    InvalidIpAddress,
    InvalidPort,
    InsufficientDiskSpace,
    InvalidDataDir,

    InvalidSizeNumber,
    InvalidSizeUnit,
}

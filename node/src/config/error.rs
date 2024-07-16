#[derive(Debug)]
pub enum ConfigError {
    InvalidIpAddress,
    InvalidPort,
    InsufficientDiskSpace,
    InvalidFragmentSize,
    InvalidSizeNumber,
    InvalidSizeUnit,
}

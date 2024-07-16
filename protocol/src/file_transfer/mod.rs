pub mod authenticator;
pub mod channel;
pub mod channel_handle;
pub mod connection;
pub mod data;
pub mod error;
pub mod handler;
mod net;
pub mod pool;
pub mod server;
mod tests;

pub const MAX_CHUNK_SIZE: u64 = 16 * 1024 * 1024;

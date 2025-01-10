use crate::framework::connection::ProtocolConnection;
use crate::mgpp::packet::MGPPPacket;

pub mod client;
pub mod error;
pub mod handler;
pub mod packet;
pub mod server;
pub mod server_handlers;
mod tests;

pub type MGPPConnection = ProtocolConnection<MGPPPacket>;

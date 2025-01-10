use crate::framework::connection::ProtocolConnection;
use crate::mgpp::packet::MGPPPacket;

pub mod client;
pub mod server;
pub mod error;
pub mod handler;
mod tests;
pub mod packet;
pub mod server_handlers;

pub type MGPPConnection = ProtocolConnection<MGPPPacket>;
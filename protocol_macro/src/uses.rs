use proc_macro2::TokenStream;
use quote::quote;

pub fn generate_uses() -> TokenStream {
    quote! {
        use std::fmt::Debug;
        use async_trait::async_trait;
        use ::protocol::framework::writer::PacketWriter;
        use ::protocol::framework::PROTOCOL_HEADER_SIZE;
        use ::protocol::framework::error::{ProtocolResult, ProtocolError};
        use ::protocol::framework::traits::{Packet, PacketDispatcher, PacketSerializer};
        use std::sync::{Arc, Weak};
        use tokio::sync::Mutex;
        use std::io::Read;
        use tokio::io::ReadHalf;
        use tokio::net::TcpStream;
        use tokio_rustls::TlsStream;
        use tokio::io::AsyncReadExt;
    }
}

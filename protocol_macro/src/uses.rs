use proc_macro2::TokenStream;
use quote::quote;

pub fn generate_uses() -> TokenStream {
    quote! {
        use std::fmt::Debug;
        use async_trait::async_trait;
        use crate::framework::writer::PacketWriter;
        use crate::framework::PROTOCOL_HEADER_SIZE;
        use commons::error::protocol_error::{ProtocolError, ProtocolResult};
        use crate::framework::traits::{Packet, PacketSerializer, PacketDispatcher};
        use std::sync::{Arc, Weak};
        use tokio::sync::Mutex;
        use tokio::io::ReadHalf;
        use tokio::net::TcpStream;
        use tokio_rustls::TlsStream;
        use tokio::io::AsyncReadExt;
        use tokio_util::sync::CancellationToken;
    }
}

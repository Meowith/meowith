use protocol_macro::Protocol;

#[derive(Protocol, Debug, Clone)]
pub enum MGPPPacket {
    InvalidateCache {
        cache_id: u32,
        cache_key: Vec<u8>
    },
}

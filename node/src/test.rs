use protocol_macro::Protocol;
use uuid::Uuid;

// Note: test enum, remove before git squash
#[derive(Protocol, Debug)]
enum MyPacket {
    A { a: i32, b: u32 },
    B { g: u32, c: Uuid },
    C,
    D { bytes: Vec<u8> },
    E { bytes: Vec<u8>, other: u32 },
    F { data: u16, str: String, datum: i32 },
}

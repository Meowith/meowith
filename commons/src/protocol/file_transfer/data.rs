pub enum LockKind {
    Read,
    Write,
}

impl TryFrom<u8> for LockKind {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0u8 => Ok(LockKind::Read),
            1u8 => Ok(LockKind::Write),
            _ => Err(())
        }
    }
}
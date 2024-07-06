use std::hash::Hash;
use uuid::Uuid;

pub mod file_lock_table;
pub mod file_read_guard;
pub mod file_write_guard;
pub mod error;

pub trait KyeAbleValue<K: Sized + Eq + Hash> {
    fn new(k: &K) -> Self;
}

impl KyeAbleValue<Uuid> for () {
    fn new(_: &Uuid) -> Self {}
}
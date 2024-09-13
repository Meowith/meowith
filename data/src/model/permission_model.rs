use num_enum::{IntoPrimitive, TryFromPrimitive};
use strum::EnumIter;

#[derive(Debug, Hash, Eq, PartialEq, EnumIter, IntoPrimitive, TryFromPrimitive, Clone, Copy)]
#[repr(i8)]
pub enum UserPermission {
    Read = 1i8,
    Write = 2i8,
    Overwrite = 3i8,
    ListDirectory = 4i8,
    ListBucket = 5i8,
    Rename = 6i8,
    Delete = 7i8,
}

impl From<&UserPermission> for i8 {
    fn from(value: &UserPermission) -> i8 {
        <UserPermission as Into<i8>>::into(*value)
    }
}

pub trait IntoBit {
    fn bit(&self) -> u64;
}

impl IntoBit for UserPermission {
    fn bit(&self) -> u64 {
        let perm_i8: i8 = self.into();
        1u64 << (perm_i8 - 1)
    }
}

#[derive(Debug, Hash, Eq, PartialEq, EnumIter, IntoPrimitive, TryFromPrimitive, Clone, Copy)]
#[repr(i8)]
pub enum AppPermission {
    CreateBucket = 1i8,
    DeleteBucket = 2i8,
    ListAllTokens = 3i8,
    DeleteAllTokens = 4i8,
    ManageRoles = 5i8,
}

impl From<&AppPermission> for i8 {
    fn from(value: &AppPermission) -> i8 {
        <AppPermission as Into<i8>>::into(*value)
    }
}

impl IntoBit for AppPermission {
    fn bit(&self) -> u64 {
        let perm_i8: i8 = self.into();
        1u64 << (perm_i8 - 1)
    }
}

#[derive(Debug, Hash, Eq, PartialEq, EnumIter, IntoPrimitive, TryFromPrimitive, Clone, Copy)]
#[repr(i32)]
pub enum GlobalRole {
    User = 1i32,
    Admin = 2i32,
}

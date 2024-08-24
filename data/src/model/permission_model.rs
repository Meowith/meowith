use charybdis::scylla::CqlValue;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use scylla::_macro_internal::{
    CellWriter, ColumnType, FromCqlValError, SerializationError, SerializeCql, WrittenCellProof,
};
use scylla::cql_to_rust::FromCqlVal;
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

impl SerializeCql for UserPermission {
    fn serialize<'b>(
        &self,
        typ: &ColumnType,
        writer: CellWriter<'b>,
    ) -> Result<WrittenCellProof<'b>, SerializationError> {
        let as_i8: i8 = self.into();
        as_i8.serialize(typ, writer)
    }
}

impl FromCqlVal<CqlValue> for UserPermission {
    fn from_cql(cql_val: CqlValue) -> Result<Self, FromCqlValError> {
        match cql_val {
            CqlValue::TinyInt(val) => {
                UserPermission::try_from(val).map_err(|_| FromCqlValError::BadVal)
            }
            _ => Err(FromCqlValError::BadCqlType),
        }
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

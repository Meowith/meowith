use charybdis::scylla::CqlValue;
use scylla::_macro_internal::{
    CellWriter, ColumnType, FromCqlValError, SerializationError, SerializeCql, WrittenCellProof,
};
use scylla::cql_to_rust::FromCqlVal;

#[derive(Debug, Hash, Eq, PartialEq)]
pub enum UserPermission {
    Read,
    Write,
    Overwrite,
    ListDirectory,
    ListBucket,
    Rename,
    Delete,
    Move,
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

impl From<&UserPermission> for i8 {
    fn from(value: &UserPermission) -> Self {
        match value {
            UserPermission::Read => 1i8,
            UserPermission::Write => 2i8,
            UserPermission::Overwrite => 3i8,
            UserPermission::ListDirectory => 4i8,
            UserPermission::ListBucket => 5i8,
            UserPermission::Rename => 6i8,
            UserPermission::Delete => 7i8,
            UserPermission::Move => 8i8,
        }
    }
}

impl TryFrom<i8> for UserPermission {
    type Error = ();

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            1i8 => Ok(UserPermission::Read),
            2i8 => Ok(UserPermission::Write),
            3i8 => Ok(UserPermission::Overwrite),
            4i8 => Ok(UserPermission::ListDirectory),
            5i8 => Ok(UserPermission::ListBucket),
            6i8 => Ok(UserPermission::Rename),
            7i8 => Ok(UserPermission::Delete),
            8i8 => Ok(UserPermission::Move),
            _ => Err(()),
        }
    }
}

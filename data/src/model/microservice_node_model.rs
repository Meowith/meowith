use charybdis::macros::charybdis_model;
use charybdis::scylla::CqlValue;
use charybdis::types::{BigInt, Boolean, Inet, Text, Timestamp, Uuid};
use scylla::_macro_internal::{
    CellWriter, ColumnType, FromCqlValError, SerializationError, SerializeCql, WrittenCellProof,
};
use scylla::cql_to_rust::FromCqlVal;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = microservice_nodes,
    partition_keys = [microservice_type],
    clustering_keys = [id],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
pub struct MicroserviceNode {
    pub microservice_type: MicroserviceType,
    pub id: Uuid,
    pub max_space: Option<BigInt>, // bytes
    pub used_space: Option<BigInt>,
    pub token: Text,
    pub address: Inet,
    pub created: Timestamp,
    pub register_code: Text,
}

#[charybdis_model(
    table_name = service_register_codes,
    partition_keys = [code],
    clustering_keys = [],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
pub struct ServiceRegisterCode {
    pub code: Text,
    pub created: Timestamp,
    pub valid: Boolean,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum MicroserviceType {
    StorageNode,
    PanelApi,
}

impl SerializeCql for MicroserviceType {
    fn serialize<'b>(
        &self,
        typ: &ColumnType,
        writer: CellWriter<'b>,
    ) -> Result<WrittenCellProof<'b>, SerializationError> {
        let as_i8: i8 = self.into();
        as_i8.serialize(typ, writer)
    }
}

impl FromCqlVal<CqlValue> for MicroserviceType {
    fn from_cql(cql_val: CqlValue) -> Result<Self, FromCqlValError> {
        match cql_val {
            CqlValue::TinyInt(val) => {
                MicroserviceType::try_from(val).map_err(|_| FromCqlValError::BadVal)
            }
            _ => Err(FromCqlValError::BadCqlType),
        }
    }
}

impl From<&MicroserviceType> for i8 {
    fn from(value: &MicroserviceType) -> Self {
        match value {
            MicroserviceType::StorageNode => 1i8,
            MicroserviceType::PanelApi => 2i8,
        }
    }
}

impl TryFrom<i8> for MicroserviceType {
    type Error = ();

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            1i8 => Ok(MicroserviceType::StorageNode),
            2i8 => Ok(MicroserviceType::PanelApi),
            _ => Err(()),
        }
    }
}

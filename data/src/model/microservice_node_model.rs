use crate::error::DataResponseError;
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpMessage, HttpRequest};
use charybdis::macros::charybdis_model;
use charybdis::scylla::CqlValue;
use charybdis::types::{BigInt, Boolean, Inet, Text, Timestamp, Uuid};
use scylla::_macro_internal::{
    CellWriter, ColumnType, FromCqlValError, SerializationError, SerializeCql, WrittenCellProof,
};
use scylla::cql_to_rust::FromCqlVal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[charybdis_model(
    table_name = microservice_nodes,
    partition_keys = [microservice_type],
    clustering_keys = [id],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
#[derive(Debug, Clone)]
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

impl Default for MicroserviceNode {
    fn default() -> Self {
        MicroserviceNode {
            microservice_type: MicroserviceType::StorageNode,
            id: Default::default(),
            max_space: None,
            used_space: None,
            token: "".to_string(),
            address: Inet::from_str("0.0.0.0").unwrap(),
            created: Default::default(),
            register_code: "".to_string(),
        }
    }
}

impl SerializeCql for MicroserviceType {
    fn serialize<'b>(
        &self,
        typ: &ColumnType,
        writer: CellWriter<'b>,
    ) -> Result<WrittenCellProof<'b>, SerializationError> {
        let as_i8: i8 = self.into();
        SerializeCql::serialize(&as_i8, typ, writer)
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

impl FromRequest for MicroserviceNode {
    type Error = DataResponseError;
    type Future = futures::future::Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        return match req.extensions().get::<MicroserviceNode>() {
            Some(node) => futures::future::ok(node.clone()),
            None => futures::future::err(DataResponseError::BadAuth),
        };
    }
}

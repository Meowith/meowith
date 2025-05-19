use crate::error::DataResponseError;
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpMessage, HttpRequest};
use charybdis::macros::charybdis_model;
use charybdis::types::{BigInt, Boolean, Inet, Text, Timestamp, TinyInt, Uuid};
use scylla::errors::PagerExecutionError;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub type MicroserviceNodeItem = Result<MicroserviceNode, PagerExecutionError>;

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
    pub microservice_type: TinyInt,
    pub id: Uuid,
    pub max_space: Option<BigInt>, // bytes
    pub used_space: Option<BigInt>,
    pub access_token: Option<Text>,
    pub access_token_issued_at: Timestamp,
    pub renewal_token: Text,
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum MicroserviceType {
    StorageNode,
    Dashboard,
}

impl Default for MicroserviceNode {
    fn default() -> Self {
        MicroserviceNode {
            microservice_type: MicroserviceType::StorageNode.into(),
            id: Default::default(),
            max_space: None,
            used_space: None,
            access_token: None,
            access_token_issued_at: Default::default(),
            renewal_token: "".to_string(),
            address: Inet::from_str("0.0.0.0").unwrap(),
            created: Default::default(),
            register_code: "".to_string(),
        }
    }
}

impl From<&MicroserviceType> for i8 {
    fn from(value: &MicroserviceType) -> Self {
        match value {
            MicroserviceType::StorageNode => 1i8,
            MicroserviceType::Dashboard => 2i8,
        }
    }
}

impl From<MicroserviceType> for i8 {
    fn from(value: MicroserviceType) -> Self {
        match value {
            MicroserviceType::StorageNode => 1i8,
            MicroserviceType::Dashboard => 2i8,
        }
    }
}

impl TryFrom<i8> for MicroserviceType {
    type Error = ();

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            1i8 => Ok(MicroserviceType::StorageNode),
            2i8 => Ok(MicroserviceType::Dashboard),
            _ => Err(()),
        }
    }
}

impl FromRequest for MicroserviceNode {
    type Error = DataResponseError;
    type Future = futures::future::Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        match req.extensions().get::<MicroserviceNode>() {
            Some(node) => futures::future::ok(node.clone()),
            None => futures::future::err(DataResponseError::BadAuth),
        }
    }
}

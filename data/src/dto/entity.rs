use crate::model::app_model::App;
use crate::model::file_model::Bucket;
use charybdis::types::{BigInt, Boolean, Text, Timestamp};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct EntityList {
    pub entities: Vec<Entity>,
}

#[derive(Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub dir: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub dir_id: Option<Uuid>,
    pub size: u64,
    pub is_dir: bool,
    pub created: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct AppDto {
    pub id: Uuid,
    pub name: String,
    pub quota: i64,
    pub created: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
}

impl From<App> for AppDto {
    fn from(value: App) -> Self {
        AppDto {
            id: value.id,
            name: value.name,
            quota: value.quota,
            created: value.created,
            last_modified: value.last_modified,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct BucketDto {
    pub app_id: Uuid,
    pub id: Uuid,
    pub name: Text,
    pub encrypted: Boolean,
    pub atomic_upload: Boolean,
    pub quota: BigInt,
    pub file_count: BigInt,
    pub space_taken: BigInt,
    pub created: Timestamp,
    pub last_modified: Timestamp,
}

impl From<Bucket> for BucketDto {
    fn from(value: Bucket) -> Self {
        BucketDto {
            app_id: value.app_id,
            id: value.id,
            name: value.name,
            encrypted: value.encrypted,
            atomic_upload: value.atomic_upload,
            quota: value.quota,
            file_count: value.file_count,
            space_taken: value.space_taken,
            created: value.created,
            last_modified: value.last_modified,
        }
    }
}

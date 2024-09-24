use crate::model::app_model::{App, AppByOwner, AppMember, AppToken, MemberByUser, UserRole};
use crate::model::file_model::Bucket;
use crate::model::microservice_node_model::ServiceRegisterCode;
use crate::model::user_model::User;
use charybdis::types::{BigInt, Boolean, Text, Timestamp};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::IpAddr;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntityList {
    pub entities: Vec<Entity>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppDto {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub quota: i64,
    pub created: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MemberDto {
    pub app_id: Uuid,
    pub member_id: Uuid,
    pub member_roles: HashSet<String>,
}

impl From<AppMember> for MemberDto {
    fn from(value: AppMember) -> Self {
        MemberDto {
            app_id: value.app_id,
            member_id: value.member_id,
            member_roles: value.member_roles,
        }
    }
}

impl From<MemberByUser> for MemberDto {
    fn from(value: MemberByUser) -> Self {
        MemberDto {
            app_id: value.app_id,
            member_id: value.member_id,
            member_roles: value.member_roles,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MemberedApp {
    pub app: AppDto,
    pub member: MemberDto,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppList {
    pub owned: Vec<AppDto>,
    pub member_of: Vec<MemberedApp>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BucketList {
    pub buckets: Vec<BucketDto>,
}

impl From<App> for AppDto {
    fn from(value: App) -> Self {
        AppDto {
            id: value.id,
            owner_id: value.owner_id,
            name: value.name,
            quota: value.quota,
            created: value.created,
            last_modified: value.last_modified,
        }
    }
}

impl From<AppByOwner> for AppDto {
    fn from(value: AppByOwner) -> Self {
        AppDto {
            id: value.id,
            owner_id: value.owner_id,
            name: value.name,
            quota: value.quota,
            created: value.created,
            last_modified: value.last_modified,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UploadSessionStartResponse {
    /// To be used in the path
    pub code: String,
    /// Seconds till the unfinished chunk is dropped when the upload is not reinitialized
    pub validity: u32,
    /// The amount already uploaded to meowith.
    /// The client should resume uploading from there.
    pub uploaded: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UploadSessionRequest {
    /// Entry size in bytes
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UploadSessionResumeResponse {
    /// The number of bytes already uploaded to the meowith store.
    pub uploaded_size: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UploadSessionResumeRequest {
    pub session_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppRolePath {
    pub name: String,
    pub app_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserRoleResponse {
    pub roles: Vec<UserRoleDto>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserRoleDto {
    pub app_id: Uuid,
    pub name: String,
    pub scopes: Vec<ScopedPermission>,
    pub created: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
}

impl From<UserRole> for UserRoleDto {
    fn from(value: UserRole) -> Self {
        UserRoleDto {
            app_id: value.app_id,
            name: value.name,
            scopes: value
                .scopes
                .map(|it| {
                    it.into_iter()
                        .map(|v| ScopedPermission {
                            bucket_id: v.0,
                            allowance: v.1 as u64,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            created: value.created,
            last_modified: value.last_modified,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScopedPermission {
    pub bucket_id: Uuid,
    pub allowance: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModifyRoleRequest {
    pub perms: Vec<ScopedPermission>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MemberIdRequest {
    pub app_id: Uuid,
    pub id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MemberRoleRequest {
    pub roles: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenDeleteRequest {
    pub app_id: Uuid,
    pub issuer_id: Uuid,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenListRequest {
    pub app_id: Uuid,
    pub issuer: Option<Uuid>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppTokenDTO {
    pub created: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub issuer_id: Uuid,
    pub name: String,
}

impl From<AppToken> for AppTokenDTO {
    fn from(value: AppToken) -> Self {
        AppTokenDTO {
            created: value.created,
            last_modified: value.last_modified,
            issuer_id: value.issuer_id,
            name: value.name,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenListResponse {
    pub tokens: Vec<AppTokenDTO>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddMemberRequest {
    pub app_id: Uuid,
    pub member_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenIssueRequest {
    pub app_id: Uuid,
    pub name: String,
    pub perms: Vec<ScopedPermission>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenameEntityRequest {
    pub to: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeleteDirectoryRequest {
    pub recursive: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeStatusResponse {
    pub nodes: Vec<NodeStatus>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeStatus {
    pub microservice_type: i8,
    pub id: Uuid,
    pub address: IpAddr,
    pub max_space: Option<u64>,
    pub used_space: Option<u64>,
    pub created: DateTime<Utc>,
    pub last_beat: DateTime<Utc>,
    pub access_token_issued_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OwnUserInfo {
    pub id: Uuid,
    pub name: String,
    pub global_role: i32,
    pub created: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
}

impl From<User> for OwnUserInfo {
    fn from(value: User) -> Self {
        OwnUserInfo {
            id: value.id,
            name: value.name,
            global_role: value.global_role,
            created: value.created,
            last_modified: value.last_modified,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceRegisterCodeDto {
    pub code: String,
    pub created: DateTime<Utc>,
    pub valid: bool,
}

impl From<ServiceRegisterCode> for ServiceRegisterCodeDto {
    fn from(value: ServiceRegisterCode) -> Self {
        ServiceRegisterCodeDto {
            code: value.code,
            created: value.created,
            valid: value.valid,
        }
    }
}

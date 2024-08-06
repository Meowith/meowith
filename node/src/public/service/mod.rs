use commons::permission::PermissionList;
use data::model::permission_model::UserPermission;
use lazy_static::lazy_static;

pub mod durable_transfer_session_manager;
pub mod file_access_service;
pub mod file_action_service;

lazy_static! {
    static ref DELETE_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Delete]).into();
    static ref RENAME_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Rename]).into();
    static ref UPLOAD_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Write]).into();
    static ref UPLOAD_OVERWRITE_ALLOWANCE: u64 = PermissionList(vec![
        UserPermission::Write,
        UserPermission::Overwrite,
        UserPermission::Delete
    ])
    .into();
    static ref DOWNLOAD_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Read]).into();
}
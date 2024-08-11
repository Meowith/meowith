use commons::permission::PermissionList;
use data::model::permission_model::UserPermission;
use lazy_static::lazy_static;

pub mod chunk_service;
pub mod durable_transfer_session_manager;
pub mod file_access_service;
pub mod file_action_service;
pub mod file_io_service;
pub mod file_list_service;
pub mod reservation_service;

lazy_static! {
    static ref DELETE_ALLOWANCE: u64 =
        PermissionList(vec![UserPermission::Delete, UserPermission::Write]).into();
    static ref RENAME_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Rename]).into();
    static ref UPLOAD_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Write]).into();
    static ref UPLOAD_OVERWRITE_ALLOWANCE: u64 = PermissionList(vec![
        UserPermission::Write,
        UserPermission::Delete,
        UserPermission::Overwrite,
    ])
    .into();
    static ref DOWNLOAD_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Read]).into();
    static ref LIST_BUCKET_ALLOWANCE: u64 = PermissionList(vec![UserPermission::ListBucket]).into();
    static ref LIST_DIR_ALLOWANCE: u64 = PermissionList(vec![UserPermission::ListDirectory]).into();
}

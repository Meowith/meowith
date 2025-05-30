pub mod check;

use crate::permission::check::PermissionListEntryBounds;
use data::dto::entity::ScopedPermission;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppTokenPermit {
    pub bucket_id: Uuid,
    pub allowance: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppTokenData {
    pub app_id: Uuid,
    pub issuer_id: Uuid,
    pub name: String,
    pub nonce: Uuid,
    pub perms: Vec<AppTokenPermit>,
}

impl From<ScopedPermission> for AppTokenPermit {
    fn from(value: ScopedPermission) -> Self {
        AppTokenPermit {
            bucket_id: value.bucket_id,
            allowance: value.allowance,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct PermissionList<T: PermissionListEntryBounds>(pub Vec<T>);

#[cfg(test)]
mod tests {
    use crate::permission::check::check_permission;
    use crate::permission::PermissionList;
    use data::model::permission_model::{IntoBit, UserPermission};

    #[test]
    fn test_list() {
        let mut perms = vec![
            UserPermission::ListBucket,
            UserPermission::Overwrite,
            UserPermission::Rename,
        ];
        perms.sort_by_key(|k| k.bit());
        let list = PermissionList(perms);

        let encoded: u64 = (&list).into();
        let decoded: PermissionList<UserPermission> = encoded.into();

        assert_eq!(list, decoded);
    }

    #[test]
    fn test_check() {
        let allowance: u64 = PermissionList(vec![
            UserPermission::ListBucket,
            UserPermission::Overwrite,
            UserPermission::Rename,
        ])
        .into();

        let req1: u64 =
            PermissionList(vec![UserPermission::ListBucket, UserPermission::Rename]).into();
        let req2: u64 = PermissionList(vec![
            UserPermission::ListBucket,
            UserPermission::Overwrite,
            UserPermission::Rename,
        ])
        .into();
        let req3: u64 = PermissionList(vec![
            UserPermission::ListBucket,
            UserPermission::Overwrite,
            UserPermission::Rename,
            UserPermission::ListDirectory,
        ])
        .into();

        assert!(!check_permission(req1, allowance));
        assert!(check_permission(req2, allowance));
        assert!(check_permission(req3, allowance));
        assert!(check_permission(127, 2));
    }
}

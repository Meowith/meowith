use crate::permission::PermissionList;
use data::model::permission_model::UserPermission;
use strum::IntoEnumIterator;

pub fn check_permission(allowance: u64, requested: u64) -> bool {
    (allowance & requested) == allowance
}

impl From<&PermissionList> for u64 {
    fn from(value: &PermissionList) -> Self {
        let mut val = 0u64;
        for perm in &value.0 {
            val |= perm.bit();
        }
        val
    }
}

impl From<PermissionList> for u64 {
    fn from(value: PermissionList) -> Self {
        (&value).into()
    }
}

impl From<u64> for PermissionList {
    fn from(value: u64) -> Self {
        let mut val = vec![];
        for perm in UserPermission::iter() {
            if value & perm.bit() != 0 {
                val.push(perm);
            }
        }
        PermissionList(val)
    }
}

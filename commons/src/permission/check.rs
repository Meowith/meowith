use crate::permission::PermissionList;
use data::model::permission_model::IntoBit;
use strum::IntoEnumIterator;

pub trait PermissionListEntryBounds: IntoBit + IntoEnumIterator {}
impl<T> PermissionListEntryBounds for T where T: IntoBit + IntoEnumIterator {}

pub fn check_permission(allowance: u64, requested: u64) -> bool {
    (allowance & requested) == requested
}

impl<T: PermissionListEntryBounds> From<&PermissionList<T>> for u64 {
    fn from(value: &PermissionList<T>) -> Self {
        let mut val = 0u64;
        for perm in &value.0 {
            val |= perm.bit();
        }
        val
    }
}

impl<T: PermissionListEntryBounds> From<PermissionList<T>> for u64 {
    fn from(value: PermissionList<T>) -> Self {
        (&value).into()
    }
}

impl<T: PermissionListEntryBounds> From<u64> for PermissionList<T> {
    fn from(value: u64) -> Self {
        let mut val = vec![];
        for perm in T::iter() {
            if value & perm.bit() != 0 {
                val.push(perm);
            }
        }
        PermissionList(val)
    }
}

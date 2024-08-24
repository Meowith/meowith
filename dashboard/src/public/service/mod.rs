use commons::error::std_response::{NodeClientError, NodeClientResponse};
use commons::permission::check::check_permission;
use commons::permission::PermissionList;
use data::access::app_access::{get_app_member, get_app_roles, UserRoleItem};
use data::error::MeowithDataError;
use data::model::app_model::App;
use data::model::permission_model::AppPermission;
use data::model::user_model::User;
use futures::StreamExt;
use lazy_static::lazy_static;
use scylla::CachingSession;
use std::collections::{HashMap, HashSet};

pub mod application_service;
pub mod bucket_service;
pub mod token_service;

lazy_static! {
    static ref CREATE_BUCKET_ALLOWANCE: u64 =
        PermissionList(vec![AppPermission::CreateBucket]).into();
    static ref DELETE_BUCKET_ALLOWANCE: u64 =
        PermissionList(vec![AppPermission::CreateBucket]).into();
}

pub enum PermCheckScope {
    Application,
    Buckets,
}

pub async fn has_app_permission(
    user: &User,
    app: &App,
    requested: u64,
    session: &CachingSession,
    scope: PermCheckScope,
) -> NodeClientResponse<()> {
    // user is owner
    if app.owner_id == user.id {
        return Ok(());
    }
    // TODO caching member & roles
    let member = get_app_member(app.id, user.id, session).await?;
    let roles: HashMap<String, HashSet<(String, i64)>> = get_app_roles(app.id, session)
        .await?
        .collect::<Vec<UserRoleItem>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(MeowithDataError::from)?
        .into_iter()
        .map(|entry| (entry.name, entry.scopes))
        .collect();

    for role in member.member_roles {
        let permits = roles.get(&role);
        if let Some(permits) = permits {
            for permit in permits {
                match scope {
                    PermCheckScope::Application => {
                        if permit.0.is_empty() && check_permission(permit.1 as u64, requested) {
                            return Ok(());
                        }
                    }
                    PermCheckScope::Buckets => {
                        if !permit.0.is_empty() && check_permission(permit.1 as u64, requested) {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Err(NodeClientError::BadAuth)
}

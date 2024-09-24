use crate::public::service::{
    has_app_permission, PermCheckScope, MANAGE_ROLES_ALLOWANCE, NO_ALLOWANCE,
};
use actix_web::web;
use chrono::Utc;
use commons::error::std_response::NodeClientError::BadRequest;
use commons::error::std_response::NodeClientResponse;
use data::access::app_access::{
    delete_app_role, get_app_by_id, get_app_member, get_app_role, get_app_roles, insert_app_role,
    update_app_member, update_app_role, UserRoleItem,
};
use data::dto::entity::{
    AppRolePath, MemberIdRequest, MemberRoleRequest, ModifyRoleRequest, UserRoleResponse,
};
use data::error::MeowithDataError;
use data::model::app_model::UserRole;
use data::model::user_model::User;
use futures_util::StreamExt;
use scylla::CachingSession;
use std::collections::HashSet;
use uuid::Uuid;

pub async fn do_get_roles(
    req: Uuid,
    user: User,
    session: &CachingSession,
) -> NodeClientResponse<web::Json<UserRoleResponse>> {
    let app = get_app_by_id(req, session).await?;
    has_app_permission(
        &user,
        &app,
        *NO_ALLOWANCE,
        session,
        PermCheckScope::Application,
    )
    .await?;

    let roles = get_app_roles(req, session)
        .await?
        .try_collect()
        .await
        .map_err(MeowithDataError::from)?;
    Ok(web::Json(UserRoleResponse {
        roles: roles.into_iter().map(|x| x.into()).collect(),
    }))
}

pub async fn do_create_role(
    req: AppRolePath,
    user: User,
    session: &CachingSession,
) -> NodeClientResponse<()> {
    let app = get_app_by_id(req.app_id, session).await?;
    has_app_permission(
        &user,
        &app,
        *MANAGE_ROLES_ALLOWANCE,
        session,
        PermCheckScope::Application,
    )
    .await?;

    let now = Utc::now();
    let role = UserRole {
        app_id: req.app_id,
        name: req.name,
        scopes: Default::default(),
        created: now,
        last_modified: now,
    };
    insert_app_role(role, session).await?;
    Ok(())
}

pub async fn do_delete_role(
    req: AppRolePath,
    user: User,
    session: &CachingSession,
) -> NodeClientResponse<()> {
    let app = get_app_by_id(req.app_id, session).await?;
    has_app_permission(
        &user,
        &app,
        *MANAGE_ROLES_ALLOWANCE,
        session,
        PermCheckScope::Application,
    )
    .await?;

    let role = get_app_role(req.app_id, req.name, session).await?;
    delete_app_role(role, session).await?;
    Ok(())
}

pub async fn do_patch_role(
    req: AppRolePath,
    user: User,
    perms: ModifyRoleRequest,
    session: &CachingSession,
) -> NodeClientResponse<()> {
    let app = get_app_by_id(req.app_id, session).await?;
    has_app_permission(
        &user,
        &app,
        *MANAGE_ROLES_ALLOWANCE,
        session,
        PermCheckScope::Application,
    )
    .await?;

    let now = Utc::now();
    let mut role = get_app_role(req.app_id, req.name, session).await?;
    role.last_modified = now;
    role.scopes = Some(
        perms
            .perms
            .into_iter()
            .map(|it| (it.bucket_id, it.allowance as i64))
            .collect(),
    );

    update_app_role(role, session).await?;
    Ok(())
}

pub async fn do_patch_member_roles(
    req: MemberIdRequest,
    user: User,
    perms: MemberRoleRequest,
    session: &CachingSession,
) -> NodeClientResponse<()> {
    let app = get_app_by_id(req.app_id, session).await?;
    has_app_permission(
        &user,
        &app,
        *MANAGE_ROLES_ALLOWANCE,
        session,
        PermCheckScope::Application,
    )
    .await?;

    let app_roles = get_app_roles(req.app_id, session)
        .await?
        .collect::<Vec<UserRoleItem>>()
        .await
        .into_iter()
        .map(|it| it.map(|it| it.name))
        .collect::<Result<HashSet<_>, _>>()
        .map_err(MeowithDataError::from)?;

    for role in &perms.roles {
        if !app_roles.contains(role) {
            return Err(BadRequest);
        }
    }

    let mut member = get_app_member(req.app_id, req.id, session).await?;
    member.member_roles = perms.roles.into_iter().collect();

    update_app_member(&member, session).await?;
    Ok(())
}

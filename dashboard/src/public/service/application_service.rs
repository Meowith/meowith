use crate::public::routes::application::{
    CreateApplicationRequest, EditApplicationQuotaRequest, EmptyResponse,
};
use crate::public::service::{has_app_permission, PermCheckScope, NO_ALLOWANCE};
use actix_web::web;
use chrono::Utc;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::access::app_access::{
    delete_app, delete_app_member, get_app_by_id, get_app_members, get_apps_by_owner,
    get_members_by_id, insert_app, insert_app_member, maybe_get_app_member, update_app_quota,
};
use data::access::file_access::{get_buckets, maybe_get_first_bucket};
use data::access::user_access::maybe_get_user_from_id;
use data::dto::entity::{AppDto, AppList, BucketList, MemberListDTO, MemberedApp};
use data::error::MeowithDataError;
use data::model::app_model::App;
use data::model::user_model::User;
use scylla::client::caching_session::CachingSession;
use uuid::Uuid;

pub async fn do_create_app(
    req: CreateApplicationRequest,
    session: &CachingSession,
    user: User,
) -> NodeClientResponse<web::Json<AppDto>> {
    let used_quota = get_user_used_quota(&user, session).await?;

    if (req.quota as i64 + used_quota) > user.quota {
        return Err(NodeClientError::InsufficientStorage {
            message: "Not enough quota for app creation".to_string(),
        });
    }

    let now = Utc::now();
    let app = App {
        id: Uuid::new_v4(),
        name: req.name,
        owner_id: user.id,
        quota: req.quota as i64,
        created: now,
        last_modified: now,
    };

    insert_app(&app, session).await?;

    Ok(web::Json(app.into()))
}

pub async fn do_edit_app(
    id: Uuid,
    req: EditApplicationQuotaRequest,
    session: &CachingSession,
    user: User,
) -> NodeClientResponse<web::Json<EmptyResponse>> {
    let used_quota = get_user_used_quota(&user, session).await?;

    let app = get_app_by_id(id, session).await?;
    let app_min = get_user_used_app_quota(&app, session).await?;

    if app_min >= req.quota as i64 {
        return Err(NodeClientError::BadRequest);
    }

    if (req.quota as i64 + used_quota - app.quota) > user.quota {
        return Err(NodeClientError::InsufficientStorage {
            message: "Not enough quota for app edition".to_string(),
        });
    }

    update_app_quota(app.id, req.quota as i64, session).await?;

    Ok(web::Json(EmptyResponse))
}

pub async fn do_delete_app(
    id: Uuid,
    session: &CachingSession,
    user: User,
) -> NodeClientResponse<()> {
    let app = get_app_by_id(id, session).await?;
    if user.id != app.owner_id {
        return Err(NodeClientError::BadAuth);
    }

    let bucket = maybe_get_first_bucket(id, session).await?;
    if bucket.is_some() {
        return Err(NodeClientError::EntityExists);
    }

    delete_app(&app, session).await?;
    Ok(())
}

pub async fn do_add_member(
    member_id: Uuid,
    app_id: Uuid,
    session: &CachingSession,
    user: User,
) -> NodeClientResponse<()> {
    let app = get_app_by_id(app_id, session).await?;
    if user.id != app.owner_id {
        return Err(NodeClientError::BadAuth);
    }
    let user = maybe_get_user_from_id(member_id, session).await?;
    if user.is_some() {
        insert_app_member(app.id, member_id, session).await?;
        Ok(())
    } else {
        Err(NodeClientError::NotFound)
    }
}

pub async fn do_delete_member(
    member_id: Uuid,
    app_id: Uuid,
    session: &CachingSession,
    user: User,
) -> NodeClientResponse<()> {
    let app = get_app_by_id(app_id, session).await?;
    if user.id != app.owner_id {
        return Err(NodeClientError::BadAuth);
    }
    let member = maybe_get_app_member(app.id, member_id, session).await?;
    if let Some(member) = member {
        delete_app_member(&member, session).await?;
        Ok(())
    } else {
        Err(NodeClientError::NotFound)
    }
}

pub async fn do_list_apps(
    user: User,
    session: &CachingSession,
) -> NodeClientResponse<web::Json<AppList>> {
    let owned_apps = get_apps_by_owner(user.id, session)
        .await?
        .try_collect()
        .await
        .map_err(MeowithDataError::from)?;

    let members = get_members_by_id(user.id, session)
        .await?
        .try_collect()
        .await
        .map_err(MeowithDataError::from)?;

    let mut member_of = Vec::new();

    for member in members {
        member_of.push(MemberedApp {
            app: get_app_by_id(member.app_id, session).await?.into(),
            member: member.into(),
        });
    }

    Ok(web::Json(AppList {
        owned: owned_apps.into_iter().map(|x| x.into()).collect(),
        member_of,
    }))
}

pub async fn do_list_buckets(
    app_id: Uuid,
    user: User,
    session: &CachingSession,
) -> NodeClientResponse<web::Json<BucketList>> {
    let app = get_app_by_id(app_id, session).await?;
    has_app_permission(
        &user,
        &app,
        *NO_ALLOWANCE,
        session,
        PermCheckScope::Application,
    )
    .await?;
    let buckets = get_buckets(app_id, session)
        .await?
        .try_collect()
        .await
        .map_err(MeowithDataError::from)?;
    Ok(web::Json(BucketList {
        buckets: buckets.into_iter().map(|x| x.into()).collect(),
    }))
}

pub async fn do_list_members(
    app_id: Uuid,
    user: User,
    session: &CachingSession,
) -> NodeClientResponse<web::Json<MemberListDTO>> {
    let app = get_app_by_id(app_id, session).await?;
    has_app_permission(
        &user,
        &app,
        *NO_ALLOWANCE,
        session,
        PermCheckScope::Application,
    )
    .await?;
    let members = get_app_members(app_id, session)
        .await?
        .try_collect()
        .await
        .map_err(MeowithDataError::from)?;

    Ok(web::Json(MemberListDTO {
        members: members.into_iter().map(|x| x.into()).collect(),
    }))
}

pub async fn get_user_used_quota(
    user: &User,
    session: &CachingSession,
) -> Result<i64, MeowithDataError> {
    let owned_apps = get_apps_by_owner(user.id, session)
        .await?
        .try_collect()
        .await
        .map_err(MeowithDataError::from)?;
    Ok(owned_apps.iter().map(|x| x.quota).sum())
}

pub async fn get_user_used_app_quota(
    app: &App,
    session: &CachingSession,
) -> Result<i64, MeowithDataError> {
    let owned_apps = get_buckets(app.id, session)
        .await?
        .try_collect()
        .await
        .map_err(MeowithDataError::from)?;
    Ok(owned_apps.iter().map(|x| x.quota).sum())
}

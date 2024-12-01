use crate::public::routes::bucket::{CreateBucketRequest, EditBucketQuotaRequest};
use crate::public::service::application_service::get_user_used_app_quota;
use crate::public::service::{
    has_app_permission, PermCheckScope, ALTER_BUCKET_ALLOWANCE, CREATE_BUCKET_ALLOWANCE,
    DELETE_BUCKET_ALLOWANCE, NO_ALLOWANCE,
};
use crate::AppState;
use actix_web::web;
use chrono::Utc;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::access::app_access::get_app_by_id;
use data::access::file_access::{
    delete_bucket, get_bucket, get_buckets, get_upload_sessions, insert_bucket,
    maybe_get_first_child_from_directory, maybe_get_first_file_from_directory, update_bucket_quota,
    BucketItem,
};
use data::dto::entity::{BucketDto, UploadSession, UploadSessionsResponse};
use data::error::MeowithDataError;
use data::model::file_model::Bucket;
use data::model::user_model::User;
use futures::StreamExt;
use scylla::CachingSession;
use uuid::Uuid;

pub async fn do_create_bucket(
    app_state: web::Data<AppState>,
    req: CreateBucketRequest,
    user: User,
) -> NodeClientResponse<web::Json<BucketDto>> {
    let app = get_app_by_id(req.app_id, &app_state.session).await?;
    has_app_permission(
        &user,
        &app,
        *CREATE_BUCKET_ALLOWANCE,
        &app_state.session,
        PermCheckScope::Application,
    )
    .await?;

    let buckets = get_buckets(req.app_id, &app_state.session)
        .await?
        .collect::<Vec<BucketItem>>()
        .await;
    let mut sum: i64 = 0;
    for bucket in buckets {
        if let Err(e) = bucket {
            return Err(MeowithDataError::from(e).into());
        } else {
            sum += bucket.unwrap().quota;
        }
    }

    if sum + req.quota as i64 > app.quota {
        return Err(NodeClientError::InsufficientStorage {
            message: "Not enough quota to create a bucket".to_string(),
        });
    }

    let now = Utc::now();
    let bucket = Bucket {
        app_id: req.app_id,
        id: Uuid::new_v4(),
        name: req.name,
        encrypted: false, // TODO
        atomic_upload: req.atomic_upload,
        quota: req.quota as i64,
        file_count: 0,
        space_taken: 0,
        created: now,
        last_modified: now,
    };

    insert_bucket(&bucket, &app_state.session).await?;

    Ok(web::Json(bucket.into()))
}

pub async fn do_delete_bucket(
    session: &CachingSession,
    app_id: Uuid,
    bucket_id: Uuid,
    user: User,
) -> NodeClientResponse<()> {
    let bucket = get_bucket(app_id, bucket_id, session).await?;
    let app = get_app_by_id(app_id, session).await?;
    has_app_permission(
        &user,
        &app,
        *DELETE_BUCKET_ALLOWANCE,
        session,
        PermCheckScope::Application,
    )
    .await?;

    if maybe_get_first_file_from_directory(bucket_id, None, session)
        .await?
        .is_some()
    {
        return Err(NodeClientError::EntityExists);
    };
    if maybe_get_first_child_from_directory(bucket_id, None, session)
        .await?
        .is_some()
    {
        return Err(NodeClientError::EntityExists);
    }

    delete_bucket(&bucket, session).await?;
    Ok(())
}

pub async fn do_edit_bucket(
    session: &CachingSession,
    req: EditBucketQuotaRequest,
    app_id: Uuid,
    bucket_id: Uuid,
    user: User,
) -> NodeClientResponse<()> {
    let mut bucket = get_bucket(app_id, bucket_id, session).await?;
    let app = get_app_by_id(app_id, session).await?;
    has_app_permission(
        &user,
        &app,
        *ALTER_BUCKET_ALLOWANCE,
        session,
        PermCheckScope::Application,
    )
    .await?;
    let reservations = get_upload_sessions(app_id, bucket_id, session).await?;
    let reserved: i64 = reservations
        .try_collect()
        .await
        .map_err(MeowithDataError::from)?
        .into_iter()
        .map(|x| x.size)
        .sum();
    let app_min = get_user_used_app_quota(&app, session).await? - bucket.quota as i64;

    if (req.quota as i64) < app_min {
        return Err(NodeClientError::InsufficientStorage {
            message: "Too little quota to edit bucket".to_string(),
        });
    }
    if (req.quota as i64) < (bucket.space_taken + reserved) {
        return Err(NodeClientError::InsufficientStorage {
            message: "Requested new quota wouldn't fit the current buckets contents".to_string(),
        });
    }

    bucket.quota = req.quota as i64;
    update_bucket_quota(&bucket, session).await?;

    Ok(())
}

pub async fn do_get_upload_sessions(
    session: &CachingSession,
    bucket_id: Uuid,
    app_id: Uuid,
    user: User,
) -> NodeClientResponse<web::Json<UploadSessionsResponse>> {
    let _ = get_bucket(app_id, bucket_id, session).await?; // Assert the bucket even exists.
    let app = get_app_by_id(app_id, session).await?;
    has_app_permission(
        &user,
        &app,
        *NO_ALLOWANCE,
        session,
        PermCheckScope::Application,
    )
    .await?;

    let sessions = get_upload_sessions(app_id, bucket_id, session)
        .await?
        .try_collect()
        .await
        .map_err(MeowithDataError::from)?;
    let sessions: Vec<UploadSession> = sessions.into_iter().map(UploadSession::from).collect();

    Ok(web::Json(UploadSessionsResponse { sessions }))
}

use crate::public::routes::bucket::CreateBucketRequest;
use crate::public::service::{has_app_permission, PermCheckScope, CREATE_BUCKET_ALLOWANCE};
use crate::AppState;
use actix_web::web;
use chrono::Utc;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::access::app_access::get_app_by_id;
use data::access::file_access::{get_buckets, insert_bucket, BucketItem};
use data::dto::entity::BucketDto;
use data::error::MeowithDataError;
use data::model::file_model::Bucket;
use data::model::user_model::User;
use futures::StreamExt;
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
        return Err(NodeClientError::InsufficientStorage);
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

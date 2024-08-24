use crate::public::routes::application::CreateApplicationRequest;
use actix_web::web;
use chrono::Utc;
use commons::error::std_response::NodeClientResponse;
use data::access::app_access::insert_app;
use data::dto::entity::AppDto;
use data::model::app_model::App;
use data::model::user_model::User;
use scylla::CachingSession;
use uuid::Uuid;

pub async fn do_create_app(
    req: CreateApplicationRequest,
    session: &CachingSession,
    user: User,
) -> NodeClientResponse<web::Json<AppDto>> {
    let now = Utc::now();
    let app = App {
        id: Uuid::new_v4(),
        name: req.name,
        owner_id: user.id,
        quota: 512 * 1024 * 1024, // TODO fetch from global conf.
        created: now,
        last_modified: now,
    };

    insert_app(&app, session).await?;

    Ok(web::Json(app.into()))
}

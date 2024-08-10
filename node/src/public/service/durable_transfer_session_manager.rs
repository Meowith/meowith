use std::ops::Deref;
use std::sync::{Arc, Weak};

use actix_web::web;
use chrono::{TimeDelta, Utc};
use futures_util::StreamExt;
use tokio::sync::RwLock;
use uuid::Uuid;

use data::access::file_access::{
    delete_upload_session_by, get_upload_session, get_upload_sessions, insert_upload_session,
    update_upload_session_last_access,
};
use data::model::file_model::BucketUploadSession;

use crate::public::response::{NodeClientError, NodeClientResponse};
use crate::AppState;

pub const DURABLE_UPLOAD_SESSION_VALIDITY_TIME_SECS: usize = 3600;

pub type SessionWeakRef = Arc<RwLock<Option<Weak<AppState>>>>;

pub struct DurableTransferSessionManager {
    session: SessionWeakRef,
}

pub async fn obtain_session(session_weak_ref: &SessionWeakRef) -> Arc<AppState> {
    session_weak_ref
        .clone()
        .read()
        .await
        .as_ref()
        .unwrap()
        .upgrade()
        .unwrap()
}

impl DurableTransferSessionManager {
    pub(crate) fn new() -> Self {
        DurableTransferSessionManager {
            session: Default::default(),
        }
    }

    pub(crate) async fn init_session(&self, app_data: web::Data<AppState>) {
        *self.session.write().await = Some(Arc::downgrade(app_data.deref()));
    }

    pub async fn start_session(&self, session: BucketUploadSession) -> NodeClientResponse<Uuid> {
        let id = session.id;
        insert_upload_session(&session, &obtain_session(&self.session).await.session).await?;

        Ok(id)
    }

    pub async fn update_session(
        &self,
        app_id: Uuid,
        bucket_id: Uuid,
        id: Uuid,
    ) -> NodeClientResponse<()> {
        let _ = update_upload_session_last_access(
            app_id,
            bucket_id,
            id,
            Utc::now(),
            &obtain_session(&self.session).await.session,
        )
        .await?;
        Ok(())
    }

    pub async fn get_session(
        &self,
        app_id: Uuid,
        bucket_id: Uuid,
        id: Uuid,
    ) -> NodeClientResponse<BucketUploadSession> {
        let remote = get_upload_session(
            app_id,
            bucket_id,
            id,
            &obtain_session(&self.session).await.session,
        )
        .await
        .map_err(|_| NodeClientError::NoSuchSession)?;

        self.update_session(app_id, bucket_id, id).await?;

        Ok(remote)
    }

    pub async fn end_session(&self, app_id: Uuid, bucket_id: Uuid, id: Uuid) {
        let _ = delete_upload_session_by(
            app_id,
            bucket_id,
            id,
            &obtain_session(&self.session).await.session,
        )
        .await;
    }

    pub async fn validate_session(&self, session: &BucketUploadSession) -> bool {
        let now = Utc::now();
        let valid = if session.durable {
            now.signed_duration_since(session.last_access)
                <= TimeDelta::seconds(DURABLE_UPLOAD_SESSION_VALIDITY_TIME_SECS as i64)
        } else {
            now.signed_duration_since(session.last_access) <= TimeDelta::seconds(2 * 60)
        };

        if !valid {
            self.end_session(session.app_id, session.bucket, session.id)
                .await
        }

        valid
    }

    pub async fn get_reserved_space(
        &self,
        app_id: Uuid,
        bucket_id: Uuid,
    ) -> NodeClientResponse<i64> {
        let mut total = 0;
        while let Some(session) = get_upload_sessions(
            app_id,
            bucket_id,
            &obtain_session(&self.session).await.session,
        )
        .await?
        .next()
        .await
        {
            let session = session.map_err(|_| NodeClientError::NoSuchSession)?;
            if self.validate_session(&session).await {
                total += session.size
            }
        }
        Ok(total)
    }
}

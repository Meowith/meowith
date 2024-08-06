use std::collections::HashMap;
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


pub struct DurableTransferSessionManager {
    session_map: Arc<RwLock<HashMap<Uuid, BucketUploadSession>>>,
    session: Arc<RwLock<Option<Weak<AppState>>>>,
}


impl DurableTransferSessionManager {
    pub(crate) fn new() -> Self {
        DurableTransferSessionManager {
            session_map: Arc::new(Default::default()),
            session: Default::default(),
        }
    }

    pub(crate) async fn init_session(&self, app_data: web::Data<AppState>) {
        *self.session.write().await = Some(Arc::downgrade(app_data.deref()));
    }

    pub async fn start_session(&self, session: BucketUploadSession) -> NodeClientResponse<Uuid> {
        let id = session.id;
        let mut map = self.session_map.write().await;
        map.insert(id, session);

        insert_upload_session(
            map.get(&id).unwrap(),
            &self
                .session
                .read()
                .await
                .as_ref()
                .unwrap()
                .upgrade()
                .unwrap()
                .session,
        )
        .await?;

        Ok(id)
    }

    pub async fn get_local_session(&self, id: &Uuid) -> NodeClientResponse<BucketUploadSession> {
        let mut map = self.session_map.write().await;

        match map.get_mut(id) {
            None => Err(NodeClientError::NoSuchSession),
            Some(entry) => {
                entry.last_access = Utc::now();
                Ok(entry.clone())
            }
        }
    }

    pub async fn get_session(
        &self,
        app_id: Uuid,
        bucket_id: Uuid,
        id: Uuid,
    ) -> NodeClientResponse<BucketUploadSession> {
        match self.get_local_session(&id).await {
            Ok(session) => Ok(session),
            Err(NodeClientError::NoSuchSession) => {
                let remote = get_upload_session(
                    app_id,
                    bucket_id,
                    id,
                    &self
                        .session
                        .read()
                        .await
                        .as_ref()
                        .unwrap()
                        .upgrade()
                        .unwrap()
                        .session,
                )
                .await
                .map_err(|_| NodeClientError::NoSuchSession)?;

                let _ = update_upload_session_last_access(
                    app_id,
                    bucket_id,
                    id,
                    Utc::now(),
                    &self
                        .session
                        .read()
                        .await
                        .as_ref()
                        .unwrap()
                        .upgrade()
                        .unwrap()
                        .session,
                )
                .await;

                Ok(remote)
            }
            Err(_) => Err(NodeClientError::InternalError),
        }
    }

    pub async fn end_local_session(&self, id: &Uuid) -> Option<BucketUploadSession> {
        let mut map = self.session_map.write().await;
        map.remove(id)
    }

    pub async fn end_session(&self, app_id: Uuid, bucket_id: Uuid, id: Uuid) {
        let _ = self.end_local_session(&id).await;
        let _ = delete_upload_session_by(
            app_id,
            bucket_id,
            id,
            &self
                .session
                .read()
                .await
                .as_ref()
                .unwrap()
                .upgrade()
                .unwrap()
                .session,
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
            &self
                .session
                .read()
                .await
                .as_ref()
                .unwrap()
                .upgrade()
                .unwrap()
                .session,
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

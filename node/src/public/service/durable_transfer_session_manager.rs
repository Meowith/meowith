use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;
use uuid::Uuid;

use data::model::file_model::FileChunk;

use crate::public::response::{NodeClientError, NodeClientResponse};
use crate::public::service::file_access::ReserveInfo;

#[allow(unused)]
pub struct DurableTransferSessionManager {
    session_map: Arc<RwLock<HashMap<Uuid, UploadSession>>>,
}

#[allow(unused)]
pub struct UploadSession {
    session: DurableReserveSession,
    when: Instant,
}

#[derive(Clone)]
#[allow(unused)]
pub struct DurableReserveInfo(Vec<FileChunk>);
#[derive(Clone)]
#[allow(unused)]
pub struct DurableReserveSession {
    pub app_id: Uuid,
    pub bucket: Uuid,
    pub path: String,
    pub size: u64,
    pub fragments: DurableReserveInfo,
}

impl From<ReserveInfo> for DurableReserveInfo {
    fn from(value: ReserveInfo) -> Self {
        DurableReserveInfo(
            (0_i8..)
                .zip(value.fragments.iter())
                .map(|chunk| FileChunk {
                    server_id: chunk.1.node_id,
                    chunk_id: chunk.1.chunk_id,
                    chunk_size: chunk.1.size as i64,
                    chunk_order: chunk.0,
                })
                .collect(),
        )
    }
}

#[allow(unused)]
impl DurableTransferSessionManager {
    pub(crate) fn new() -> Self {
        DurableTransferSessionManager {
            session_map: Arc::new(Default::default()),
        }
    }

    pub async fn start_session(&self, session: DurableReserveSession) -> NodeClientResponse<Uuid> {
        let id = Uuid::new_v4();
        let mut map = self.session_map.write().await;
        map.insert(
            id,
            UploadSession {
                session,
                when: Instant::now(),
            },
        );
        Ok(id)
    }

    pub async fn get_session(&self, id: &Uuid) -> NodeClientResponse<DurableReserveSession> {
        let mut map = self.session_map.write().await;

        match map.get_mut(id) {
            None => Err(NodeClientError::NoSuchSession),
            Some(entry) => {
                entry.when = Instant::now();
                Ok(entry.session.clone())
            }
        }
    }
}

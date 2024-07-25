use uuid::Uuid;
use crate::public::response::NodeClientResponse;

pub struct DurableTransferSessionManager {}

pub struct UploadSession {}

impl DurableTransferSessionManager {
    pub(crate) fn new() -> Self {
        DurableTransferSessionManager {}
    }

    pub async fn start_session(&self, _app_id: Uuid, _bucket: String, _path: String, _size: u64) -> NodeClientResponse<Uuid> {
        todo!()
    }
}
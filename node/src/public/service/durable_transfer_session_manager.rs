use crate::public::response::NodeClientResponse;
use uuid::Uuid;

#[allow(unused)]
pub struct DurableTransferSessionManager {}

#[allow(unused)]
pub struct UploadSession {}

#[allow(unused)]
impl DurableTransferSessionManager {
    pub(crate) fn new() -> Self {
        DurableTransferSessionManager {}
    }

    pub async fn start_session(
        &self,
        _app_id: Uuid,
        _bucket: Uuid,
        _path: String,
        _size: u64,
    ) -> NodeClientResponse<Uuid> {
        todo!()
    }
}

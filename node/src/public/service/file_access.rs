use std::sync::Arc;
use crate::file_transfer::channel_handler::{AbstractReadStream, AbstractWriteStream};
use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::{NodeClientError, NodeClientResponse};
use crate::public::routes::file_transfer::{UploadSessionRequest, UploadSessionStartResponse};
use actix_web::http::header::ContentType;
use actix_web::web;
use commons::permission::PermissionList;
use data::model::permission_model::UserPermission;
use lazy_static::lazy_static;
use protocol::mdsftp::channel_handle::ChannelAwaitHandle;
use uuid::Uuid;
use commons::context::microservice_request_context::{MicroserviceRequestContext, NodeStorageMap};
use crate::io::error::MeowithIoError;
use crate::io::fragment_ledger::FragmentLedger;
use crate::public::service::durable_transfer_session_manager::DurableTransferSessionManager;

lazy_static! {
    static ref UPLOAD_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Write]).into();
    static ref DOWNLOAD_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Read]).into();
}

pub enum ReservationMode {
    PreferSelfThenMostFree,
    PreferMostFree,
}

pub struct DlInfo {
    pub size: u64,
    pub attachment_name: String,
    pub mime: ContentType,
}

pub async fn handle_upload_oneshot(
    app_id: Uuid,
    bucket: String,
    accessor: BucketAccessor,
    _reader: AbstractReadStream,
) -> NodeClientResponse<ChannelAwaitHandle> {
    accessor.has_permission(&bucket, &app_id, *UPLOAD_ALLOWANCE)?;
    todo!()
}

pub async fn start_upload_session(
    app_id: Uuid,
    bucket: String,
    accessor: BucketAccessor,
    req: UploadSessionRequest,
    session_manager: &DurableTransferSessionManager,
) -> NodeClientResponse<web::Json<UploadSessionStartResponse>> {
    accessor.has_permission(&bucket, &app_id, *UPLOAD_ALLOWANCE).map_err(|_| NodeClientError::BadRequest)?;
    // TODO validation
    let session_id = session_manager.start_session(app_id, bucket, req.path, req.size).await?;
    Ok(web::Json(UploadSessionStartResponse {
        code: session_id.to_string(),
        validity: 0,
    }))
}

pub async fn handle_upload_durable(
    _session_id: Uuid,
    _accessor: BucketAccessor,
    _reader: AbstractReadStream,
) -> NodeClientResponse<ChannelAwaitHandle> {
    todo!()
}

pub async fn handle_download(
    app_id: Uuid,
    bucket: String,
    _path: String,
    accessor: BucketAccessor,
    _writer: AbstractWriteStream,
) -> NodeClientResponse<DlInfo> {
    accessor.has_permission(&bucket, &app_id, *DOWNLOAD_ALLOWANCE)?;
    todo!()
}

struct ReserveInfo {
    fragments: Vec<(Uuid, Uuid, u64)>
}

async fn reserve_chunks(size: u64, mode: ReservationMode, ledger: FragmentLedger, node_map: NodeStorageMap, req_ctx: Arc<MicroserviceRequestContext>) -> NodeClientResponse<ReserveInfo> {
    let mut target_list: Vec<(Uuid, u64)> = vec![];
    let self_free = ledger.get_available_space();
    let rem: u64;

    // Figure out targets
    match mode {
        ReservationMode::PreferSelfThenMostFree => {
            if self_free >= size {
                target_list.push((req_ctx.id, size));
                rem = 0;
            } else {
                rem = push_most_used(&node_map, &mut target_list, size).await;
            }
        }
        ReservationMode::PreferMostFree => {
            rem = push_most_used(&node_map, &mut target_list, size).await;
        }
    }

    if rem > 0 {
        return Err(NodeClientError::InsufficientStorage);
    }

    // Try reserve
    // TODO, cancel reserve, write to reserve at different time
    todo!()
}

async fn push_most_used(node_map: &NodeStorageMap, target_list: &mut Vec<(Uuid, u64)>, mut size: u64) -> u64 {
    let node_map_read = node_map.read().await;

    let mut nodes: Vec<(&Uuid, &u64)> = node_map_read.iter().collect();
    nodes.sort_by(|a, b| b.1.cmp(a.1));

    for (uuid, &node_size) in nodes {
        if size == 0 {
            break;
        }

        if size >= node_size {
            size -= node_size;
            target_list.push((*uuid, node_size));
        } else {
            target_list.push((*uuid, size));
            size = 0;
        }
    }

    size
}
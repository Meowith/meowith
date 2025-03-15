use crate::AppState;
use actix_web::web::Data;
use commons::context::microservice_request_context::NodeStorageMap;
use commons::error::io_error::MeowithIoError;
use commons::error::mdsftp_error::{MDSFTPError, MDSFTPResult};
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::model::file_model::FileChunk;
use protocol::mdsftp::channel::MDSFTPChannel;
use protocol::mdsftp::data::ReserveFlags;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Copy, Clone)]
pub enum ReservationMode {
    PreferSelfThenMostFree,
    PreferMostFree,
}

pub struct ReservedFragment {
    pub channel: Option<MDSFTPChannel>,
    pub node_id: Uuid,
    pub chunk_id: Uuid,
    pub size: u64,
    pub chunk_buffer: u16,
}

pub struct ReserveInfo {
    // Channel, none if local, node_id, chunk_id, size, chunk_buffer
    pub fragments: Vec<ReservedFragment>,
}

pub fn reserve_info_to_file_chunks(reserve_info: &ReserveInfo) -> HashSet<FileChunk> {
    (0_i8..)
        .zip(reserve_info.fragments.iter())
        .map(|chunk| FileChunk {
            server_id: chunk.1.node_id,
            chunk_id: chunk.1.chunk_id,
            chunk_size: chunk.1.size as i64,
            chunk_order: chunk.0,
        })
        .collect()
}

async fn resolve_targets(
    state: &Data<AppState>,
    size: u64,
    mode: ReservationMode,
) -> (Vec<(Uuid, u64)>, u64) {
    let mut target_list: Vec<(Uuid, u64)> = vec![];
    let self_free = state.fragment_ledger.get_available_space();
    let rem: u64;

    // Figure out targets
    match mode {
        ReservationMode::PreferSelfThenMostFree => {
            if self_free >= size {
                target_list.push((state.req_ctx.id, size));
                rem = 0;
            } else {
                target_list.push((state.req_ctx.id, self_free));
                rem = push_most_used(&state.node_storage_map, &mut target_list, size - self_free)
                    .await;
            }
        }
        ReservationMode::PreferMostFree => {
            rem = push_most_used(&state.node_storage_map, &mut target_list, size).await;
        }
    }
    (target_list, rem)
}

pub async fn reserve_chunks(
    size: u64,
    flags: ReserveFlags,
    associated_bucket_id: Uuid,
    associated_file_id: Uuid,
    mode: ReservationMode,
    state: &Data<AppState>,
) -> NodeClientResponse<ReserveInfo> {
    let (mut target_list, mut rem) = resolve_targets(state, size, mode).await;

    if rem > 0 {
        // In case we have stale data, try to refresh our list of other nodes and retry
        if !state.safe_refresh_peer_data().await? {
            // If no refresh has been performed, return an error.
            return Err(NodeClientError::InsufficientStorage {
                message: format!("Failed to reserve space, rem={rem}"),
            });
        }

        (target_list, rem) = resolve_targets(state, size, mode).await;

        if rem > 0 {
            return Err(NodeClientError::InsufficientStorage {
                message: format!("Failed to reserve space, rem={rem}"),
            });
        }
    }

    // Try reserve
    let mut fragments: Vec<ReservedFragment> = vec![];
    let res: MDSFTPResult<()> = async {
        for frag in target_list {
            fragments.push(
                try_reserve_chunk(
                    frag.0,
                    frag.1,
                    associated_bucket_id,
                    associated_file_id,
                    &flags,
                    state,
                )
                .await?,
            );
        }
        Ok(())
    }
    .await;

    match res {
        Ok(_) => Ok(ReserveInfo { fragments }),
        Err(_) => {
            // If any reservation fails, release the ones currently acquired
            for frag in fragments {
                if let Some(channel) = frag.channel {
                    let _ = channel.cancel_reserve(frag.chunk_id).await;
                } else {
                    let _ = state
                        .fragment_ledger
                        .cancel_reservation(&frag.chunk_id)
                        .await;
                }
            }

            Err(NodeClientError::InsufficientStorage {
                message: "Remote reservation failed".to_string(),
            })
        }
    }
}

pub async fn try_reserve_chunk(
    node_id: Uuid,
    size: u64,
    associated_bucket_id: Uuid,
    associated_file_id: Uuid,
    flags: &ReserveFlags,
    state: &Data<AppState>,
) -> MDSFTPResult<ReservedFragment> {
    let pool = state.mdsftp_server.pool();
    if node_id == state.req_ctx.id {
        let uuid = match state
            .fragment_ledger
            .try_reserve(
                size,
                associated_bucket_id,
                associated_file_id,
                flags.durable,
            )
            .await
        {
            Ok(id) => Ok(id),
            Err(MeowithIoError::InsufficientDiskSpace) => Err(MDSFTPError::ReservationError),
            Err(_) => Err(MDSFTPError::Internal),
        }?;

        Ok(ReservedFragment {
            channel: None,
            node_id,
            chunk_id: uuid,
            size,
            chunk_buffer: 0,
        })
    } else {
        let channel = pool.channel(&node_id).await?;
        let res = channel
            .try_reserve(size, associated_file_id, associated_bucket_id, *flags)
            .await;
        match res {
            Ok(res) => Ok(ReservedFragment {
                channel: Some(channel),
                node_id,
                chunk_id: res.chunk_id,
                size,
                chunk_buffer: res.chunk_buffer,
            }),
            Err(MDSFTPError::ReserveError(free_space)) => {
                let mut write = state.node_storage_map.write().await;
                write.insert(node_id, free_space);
                Err(MDSFTPError::ReservationError)
            }
            Err(e) => Err(e),
        }
    }
}

async fn push_most_used(
    node_map: &NodeStorageMap,
    target_list: &mut Vec<(Uuid, u64)>,
    mut size: u64,
) -> u64 {
    let node_map_read = node_map.read().await;

    let mut nodes: Vec<(&Uuid, &u64)> = node_map_read.iter().collect();
    nodes.sort_by(|a, b| b.1.cmp(a.1));

    for (uuid, &node_size) in nodes {
        if size == 0 {
            break;
        }

        if target_list.iter().filter(|it| it.0 == *uuid).count() > 0 {
            continue;
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

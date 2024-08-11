use crate::io::error::MeowithIoError;
use crate::public::response::{NodeClientError, NodeClientResponse};
use crate::AppState;
use actix_web::web::Data;
use commons::context::microservice_request_context::NodeStorageMap;
use data::model::file_model::FileChunk;
use protocol::mdsftp::channel::MDSFTPChannel;
use protocol::mdsftp::data::ReserveFlags;
use protocol::mdsftp::error::{MDSFTPError, MDSFTPResult};
use std::collections::HashSet;
use uuid::Uuid;

#[allow(dead_code)]
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

pub async fn reserve_chunks(
    size: u64,
    flags: ReserveFlags,
    mode: ReservationMode,
    state: &Data<AppState>,
) -> NodeClientResponse<ReserveInfo> {
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
                rem = push_most_used(&state.node_storage_map, &mut target_list, size).await;
            }
        }
        ReservationMode::PreferMostFree => {
            rem = push_most_used(&state.node_storage_map, &mut target_list, size).await;
        }
    }

    if rem > 0 {
        return Err(NodeClientError::InsufficientStorage);
    }

    // Try reserve
    let mut fragments: Vec<ReservedFragment> = vec![];
    let res: MDSFTPResult<()> = async {
        let pool = state.mdsftp_server.pool();
        for frag in target_list {
            if frag.0 == state.req_ctx.id {
                let uuid = match state
                    .fragment_ledger
                    .try_reserve(frag.1, flags.durable)
                    .await
                {
                    Ok(id) => Ok(id),
                    Err(MeowithIoError::InsufficientDiskSpace) => {
                        Err(MDSFTPError::ReservationError)
                    }
                    Err(_) => Err(MDSFTPError::Internal),
                }?;

                fragments.push(ReservedFragment {
                    channel: None,
                    node_id: frag.0,
                    chunk_id: uuid,
                    size: frag.1,
                    chunk_buffer: 0,
                });
            } else {
                let channel = pool.channel(&frag.0).await?;
                let res = channel.try_reserve(frag.1, flags).await;
                match res {
                    Ok(res) => {
                        fragments.push(ReservedFragment {
                            channel: Some(channel),
                            node_id: frag.0,
                            chunk_id: res.chunk_id,
                            size: frag.1,
                            chunk_buffer: res.chunk_buffer,
                        });
                        Ok(())
                    }
                    Err(MDSFTPError::ReserveError(free_space)) => {
                        let mut write = state.node_storage_map.write().await;
                        write.insert(frag.0, free_space);
                        Err(MDSFTPError::ReservationError)
                    }
                    Err(e) => Err(e),
                }?;
            }
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

            Err(NodeClientError::InsufficientStorage)
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

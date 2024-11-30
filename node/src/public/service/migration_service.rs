use crate::public::service::chunk_service::ChunkInfo;
use crate::public::service::file_io_service::inbound_transfer;
use crate::public::service::reservation_service::try_reserve_chunk;
use crate::AppState;
use actix_web::web::Data;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::access::file_access::{get_all_files, update_file_chunks};
use data::error::MeowithDataError;
use futures_util::StreamExt;
use log::error;
use protocol::mdsftp::data::ReserveFlags;
use rand::prelude::SliceRandom;
use std::collections::HashSet;
use uuid::Uuid;

/// Copies over a local chunk to another node
pub async fn move_chunk(
    id: Uuid,
    target_node: Uuid,
    state: &Data<AppState>,
) -> NodeClientResponse<Uuid> {
    let chunk_info = state
        .fragment_ledger
        .fragment_meta(&id)
        .await
        .ok_or(NodeClientError::NotFound)?;

    let chunk = state
        .fragment_ledger
        .fragment_read_stream(&id)
        .await
        .map_err(|_| NodeClientError::InternalError)?;

    if target_node == state.req_ctx.id {
        error!("Cannot migrate to self.");
        return Err(NodeClientError::InternalError);
    };

    let flags = ReserveFlags {
        auto_start: true,
        durable: false,
        temp: false,
        overwrite: false,
    };
    // TODO: store bucket id's and file id's in the fragment ledger
    let space = try_reserve_chunk(
        target_node,
        chunk_info.disk_content_size,
        Uuid::new_v4(),
        Uuid::new_v4(),
        &flags,
        state,
    )
    .await?;

    inbound_transfer(
        chunk,
        0,
        space.node_id,
        space.chunk_id,
        space.channel,
        ChunkInfo {
            chunk_buffer: space.chunk_buffer,
            size: space.size,
            append: false,
        },
        state,
    )
    .await?;

    Ok(space.chunk_id)
}

pub async fn migrate_chunks(
    state: &Data<AppState>,
    targets: HashSet<Uuid>,
) -> NodeClientResponse<()> {
    state.pause().await;

    let mut file_stream = get_all_files(&state.session).await?;
    while let Some(file) = file_stream.next().await {
        let mut file = file.map_err(MeowithDataError::from)?;
        let mut new_chunks = HashSet::new();
        let mut changed = false;
        let storage_map = state.node_storage_map.write().await;
        for mut chunk in file.chunk_ids {
            if state.fragment_ledger.fragment_exists(&chunk.chunk_id).await {
                let mut candidates = vec![];
                for potential_target in &targets {
                    if *storage_map.get(potential_target).unwrap_or(&0u64) as i64
                        >= chunk.chunk_size
                    {
                        candidates.push(*potential_target);
                    }
                }
                let target = *candidates.choose(&mut rand::thread_rng()).ok_or(
                    NodeClientError::InsufficientStorage {
                        message: "No suitable candidate".to_string(),
                    },
                )?;

                // In case any transfers are still ongoing, check up with the locking table.
                {
                    let _guard = state
                        .fragment_ledger
                        .lock_table()
                        .read(chunk.chunk_id)
                        .await;
                    move_chunk(chunk.chunk_id, target, state).await?;
                }
                {
                    let _guard = state
                        .fragment_ledger
                        .lock_table()
                        .write(chunk.chunk_id)
                        .await;
                    state.fragment_ledger.delete_chunk(&chunk.chunk_id).await?;
                }
                chunk.server_id = target;
                new_chunks.insert(chunk);
                changed = true;
            } else {
                new_chunks.insert(chunk);
            }
        }
        if changed {
            file.chunk_ids = new_chunks;
            update_file_chunks(&file, &state.session).await?;
        }
    }

    Ok(())
}

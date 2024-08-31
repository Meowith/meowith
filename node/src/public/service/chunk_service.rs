use crate::AppState;
use actix_web::web::Data;
use commons::error::mdsftp_error::MDSFTPError;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use log::trace;
use protocol::mdsftp::data::CommitFlags;
use uuid::Uuid;
use logging::log_err;

pub async fn commit_chunk(
    flags: CommitFlags,
    node_id: Uuid,
    chunk_id: Uuid,
    state: &Data<AppState>,
) -> NodeClientResponse<()> {
    if node_id == state.req_ctx.id {
        trace!("Trying to commit local chunk {chunk_id}");
        if flags.r#final {
            log_err("commit fail", state.fragment_ledger.commit_chunk(&chunk_id).await);
        } else if flags.keep_alive {
            log_err("commit fail", state.fragment_ledger.commit_alive(&chunk_id).await);
        } else if flags.reject {
            log_err("commit fail", state.fragment_ledger.delete_chunk(&chunk_id).await);
        }
        Ok(())
    } else {
        trace!("Trying to commit remote chunk {chunk_id}");
        let pool = state.mdsftp_server.pool();
        let channel = pool.channel(&node_id).await?;
        channel
            .commit(chunk_id, flags)
            .await
            .map_err(NodeClientError::from)?;
        Ok(())
    }
}

pub struct ChunkInfo {
    pub chunk_buffer: u16,
    pub size: u64,
    pub append: bool,
}

///
/// Fetches chunk size
/// To avoid unnecessary network calls if chunk is on the current node it just returns size right away
/// otherwise it queries the origin node for this value using MDSFTP
///
/// Returns the chunk size
///
pub async fn query_chunk(
    chunk_id: Uuid,
    node_id: Uuid,
    state: &Data<AppState>,
) -> NodeClientResponse<Option<u64>> {
    let chunk = {
        if node_id == state.req_ctx.id {
            Ok(state
                .fragment_ledger
                .fragment_meta_ex(&chunk_id)
                .await
                .map(|c| c.disk_content_size))
        } else {
            let pool = state.mdsftp_server.pool();
            let channel = pool.channel(&node_id).await?;
            match channel.query_chunk(chunk_id).await {
                Ok(res) => Ok(Some(res.size)),
                Err(MDSFTPError::NoSuchChunkId) => Ok(None),
                Err(e) => Err(e)?,
            }
        }
    };

    trace!("Query chunk {chunk_id} on {node_id} = {chunk:?}");
    chunk
}

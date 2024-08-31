use crate::file_transfer::channel_handler::MeowithMDSFTPChannelPacketHandler;
use crate::public::service::chunk_service::ChunkInfo;
use crate::AppState;
use actix_web::web::Data;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use log::trace;
use protocol::mdsftp::channel::MDSFTPChannel;
use protocol::mdsftp::data::{ChunkRange, PutFlags};
use protocol::mdsftp::handler::{AbstractReadStream, AbstractWriteStream};
use std::io::SeekFrom;
use std::pin::Pin;
use tokio::io;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use uuid::Uuid;

pub async fn inbound_transfer(
    reader: AbstractReadStream,
    skip: u64,
    node_id: Uuid,
    chunk_id: Uuid,
    channel: Option<MDSFTPChannel>,
    chunk: ChunkInfo,
    state: &Data<AppState>,
) -> NodeClientResponse<()> {
    if node_id == state.req_ctx.id {
        trace!("Inbound transfer to current");
        let res: NodeClientResponse<()> = async {
            let writer = if chunk.append {
                state
                    .fragment_ledger
                    .fragment_append_stream(&chunk_id)
                    .await
            } else {
                state.fragment_ledger.fragment_write_stream(&chunk_id).await
            }
            .map_err(|_| NodeClientError::InternalError)?;

            let mut writer = writer.lock().await;
            let reader = reader.lock().await;
            let mut reader = Pin::new(reader).take(chunk.size - skip);
            let copied = io::copy(&mut reader, &mut *writer)
                .await
                .map_err(|_| NodeClientError::InternalError)?;
            if copied != chunk.size - skip {
                return Err(NodeClientError::BadRequest);
            }
            Ok(())
        }
        .await;
        match res {
            Ok(_) => {
                trace!("releasing completed transfer");
                state
                    .fragment_ledger
                    .release_reservation(&chunk_id, chunk.size)
                    .await
                    .map_err(|_| NodeClientError::InternalError)?;
                Ok(())
            }
            Err(e) => {
                trace!("releasing interrupted transfer {e}");
                let size = state.fragment_ledger.stat_chunk(&chunk_id).await?;
                state
                    .fragment_ledger
                    .release_reservation(&chunk_id, size)
                    .await
                    .map_err(|_| NodeClientError::InternalError)?;
                Err(e)
            }
        }
    } else {
        trace!("Inbound transfer to remote");
        let eff_channel: MDSFTPChannel;
        let mut eff_chunk_buf: u16 = chunk.chunk_buffer;
        let pool = state.mdsftp_server.pool();
        match channel {
            None => {
                let channel = pool.channel(&node_id).await?;
                let res = channel
                    .request_put(
                        PutFlags {
                            append: chunk.append,
                        },
                        chunk_id,
                        chunk.size - skip,
                    )
                    .await?;
                eff_chunk_buf = res.chunk_buffer;
                eff_channel = channel;
            }
            Some(c) => eff_channel = c,
        }

        let handler = Box::new(MeowithMDSFTPChannelPacketHandler::new(
            state.fragment_ledger.clone(),
            pool.cfg.buffer_size,
            pool.cfg.fragment_size,
        ));
        let handle = eff_channel
            .send_content(reader, chunk.size - skip, eff_chunk_buf, handler)
            .await?;

        handle.await.map_or(Ok(()), |e| e)?;
        Ok(())
    }
}

pub async fn outbound_transfer(
    writer: AbstractWriteStream,
    node_id: Uuid,
    chunk_id: Uuid,
    state: &Data<AppState>,
    range: Option<ChunkRange>,
) -> NodeClientResponse<()> {
    if node_id == state.req_ctx.id {
        // send local chunk, no need for net io
        let mut reader = state
            .fragment_ledger
            .raw_fragment_read_omni_stream(&chunk_id)
            .await
            .map_err(|_| NodeClientError::NotFound)?;
        let mut writer = writer.lock().await;

        if let Some(range) = range {
            reader.seek(SeekFrom::Start(range.start)).await?;
            io::copy(&mut reader.take(range.size()), &mut *writer)
                .await
                .map_err(|_| NodeClientError::InternalError)?;
        } else {
            io::copy(&mut reader, &mut *writer)
                .await
                .map_err(|_| NodeClientError::InternalError)?;
        }

        Ok(())
    } else {
        let pool = state.mdsftp_server.pool();
        // send remote chunk
        let channel = pool.channel(&node_id).await?;
        let handler = Box::new(MeowithMDSFTPChannelPacketHandler::new(
            state.fragment_ledger.clone(),
            pool.cfg.buffer_size,
            pool.cfg.fragment_size,
        ));

        let handle = channel
            .retrieve_content(writer, handler, false) // there might be more chunks to send!
            .await?;

        channel.retrieve_req(chunk_id, 16, range).await?;

        handle
            .await
            .map_or(Ok(()), |e| e.map_err(NodeClientError::from))
    }
}

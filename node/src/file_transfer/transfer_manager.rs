use protocol::file_transfer::error::{MDSFTPError, MDSFTPResult};
use protocol::file_transfer::handler::Channel;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, BufReader};
use tokio::sync::mpsc::Receiver;
use tokio::sync::MutexGuard;

/// Sends a file to different node
pub async fn mdsftp_upload<T: AsyncRead + Unpin>(
    channel: &Channel,
    mut read: MutexGuard<'_, BufReader<T>>,
    size: u64,
    mut receiver: Receiver<u32>,
    chunk_buffer: u16,
    transfer_tracker: Arc<AtomicU64>,
    fragment_size: u32,
) -> MDSFTPResult<()> {
    let fragment_size = fragment_size as u64;
    let mut recv: u32 = 0;
    let fragments = size / fragment_size;
    if fragments > (u32::MAX - 1) as u64 {
        return Err(MDSFTPError::Interrupted);
    }
    let fragments = fragments as u32;
    let last = size % fragment_size;
    let mut upload_buffer = vec![0u8; fragment_size as usize];

    for id in 0..fragments {
        read.read_exact(&mut upload_buffer)
            .await
            .map_err(MDSFTPError::from)?;
        channel
            .respond_chunk(id + 1 == fragments && last == 0, id, &upload_buffer)
            .await?;

        transfer_tracker.fetch_add(fragment_size, Ordering::SeqCst);

        while id >= recv + chunk_buffer as u32 {
            recv = receiver.recv().await.ok_or(MDSFTPError::Interrupted)?;
        }
    }

    // If fragments == 0 then the entire content is smaller than one chunk, send it in whole.
    let send_last = last != 0 || fragments == 0;

    if send_last {
        read.read_exact(&mut upload_buffer[0..last as usize])
            .await
            .map_err(MDSFTPError::from)?;

        channel
            .respond_chunk(true, fragments, &upload_buffer[0..last as usize])
            .await?;

        transfer_tracker.fetch_add(last, Ordering::SeqCst);
    }

    let sent = fragments + if send_last { 1 } else { 0 };

    // Await transfer completion.
    while recv + 1 != sent + 1 {
        recv = receiver.recv().await.ok_or(MDSFTPError::Interrupted)? + 1;
    }

    Ok(())
}

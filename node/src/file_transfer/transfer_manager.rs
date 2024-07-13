use protocol::file_transfer::error::{MDSFTPError, MDSFTPResult};
use protocol::file_transfer::handler::Channel;
use protocol::file_transfer::MAX_CHUNK_SIZE;
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
) -> MDSFTPResult<()> {
    let mut recv: u32 = 0;
    let fragments = size / MAX_CHUNK_SIZE;
    if fragments > (u32::MAX - 1) as u64 {
        return Err(MDSFTPError::Interrupted);
    }
    let fragments = fragments as u32;
    let last = size % MAX_CHUNK_SIZE;
    let mut upload_buffer = [0u8; MAX_CHUNK_SIZE as usize];

    for id in 0..fragments {
        read.read_exact(&mut upload_buffer)
            .await
            .map_err(MDSFTPError::from)?;
        channel
            .respond_chunk(id + 1 == fragments && last == 0, id, &upload_buffer)
            .await?;

        while id >= recv + chunk_buffer as u32 {
            recv = receiver.recv().await.ok_or(MDSFTPError::Interrupted)?;
        }
    }

    if last != 0 || fragments == 0 {
        read.read_exact(&mut upload_buffer[0..last as usize])
            .await
            .map_err(MDSFTPError::from)?;

        channel
            .respond_chunk(true, fragments, &upload_buffer[0..last as usize])
            .await?;
    }

    let sent = fragments + if last != 0 { 1 } else { 0 };

    // Await transfer completion.
    while recv + 1 != sent {
        recv = receiver.recv().await.ok_or(MDSFTPError::Interrupted)?;
    }

    Ok(())
}

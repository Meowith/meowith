use commons::error::mdsftp_error::{MDSFTPError, MDSFTPResult};
use log::warn;
use protocol::mdsftp::data::ChunkRange;
use protocol::mdsftp::handler::Channel;
use protocol::mdsftp::handler::{AbstractFileStream, AbstractReadStream};
use std::io::SeekFrom;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::mpsc::Receiver;
use tokio_util::either::Either;

pub(crate) struct SizeParams {
    pub(crate) size: u64,
    pub(crate) range: Option<ChunkRange>,
}

impl SizeParams {
    fn size(&self) -> u64 {
        if self.range.is_some() {
            self.range.as_ref().unwrap().size()
        } else {
            self.size
        }
    }

    fn offset(&self) -> u64 {
        self.range.as_ref().map(|it| it.start).unwrap_or(0)
    }
}

macro_rules! transfer_fragments {
    (
        $read:expr,
        $channel:expr,
        $upload_buffer:expr,
        $fragment_size:expr,
        $last:expr,
        $fragments:expr,
        $recv:expr,
        $chunk_buffer:expr,
        $receiver:expr,
        $transfer_tracker:expr
    ) => {
        for id in 0..$fragments {
            $read
                .read_exact(&mut $upload_buffer)
                .await
                .map_err(MDSFTPError::from)?;
            $channel
                .respond_chunk(id + 1 == $fragments && $last == 0, id, &$upload_buffer)
                .await?;

            $transfer_tracker.fetch_add($fragment_size, Ordering::SeqCst);

            while id >= $recv + $chunk_buffer as u32 {
                $recv = $receiver.recv().await.ok_or(MDSFTPError::Interrupted)?;
            }
        }

        let send_last = $last != 0 || $fragments == 0;

        if send_last {
            $read
                .read_exact(&mut $upload_buffer[0..$last as usize])
                .await
                .map_err(MDSFTPError::from)?;

            $channel
                .respond_chunk(true, $fragments, &$upload_buffer[0..$last as usize])
                .await?;

            $transfer_tracker.fetch_add($last, Ordering::SeqCst);
        }

        let sent = $fragments + if send_last { 1 } else { 0 };

        while $recv + 1 != sent + 1 {
            $recv = $receiver.recv().await.ok_or(MDSFTPError::Interrupted)? + 1;
        }
    };
}

pub async fn mdsftp_upload(
    channel: &Channel,
    read: Either<AbstractReadStream, AbstractFileStream>,
    size_params: SizeParams,
    mut receiver: Receiver<u32>,
    chunk_buffer: u16,
    transfer_tracker: Arc<AtomicU64>,
    fragment_size: u32,
) -> MDSFTPResult<()> {
    let size = size_params.size();
    let offset = size_params.offset();

    let fragment_size = fragment_size as u64;
    let mut recv: u32 = 0;
    let fragments = size / fragment_size;
    if fragments > (u32::MAX - 1) as u64 {
        warn!("Failed to send, too many fragments");
        return Err(MDSFTPError::Interrupted);
    }
    let fragments = fragments as u32;
    let last = size % fragment_size;
    let mut upload_buffer = vec![0u8; fragment_size as usize];

    match read {
        Either::Left(read_stream) => {
            if offset > 0 {
                warn!("Attempted to seek on a non seekable stream");
            }

            let mut read_stream = read_stream.lock().await;
            transfer_fragments!(
                read_stream,
                channel,
                upload_buffer,
                fragment_size,
                last,
                fragments,
                recv,
                chunk_buffer,
                receiver,
                transfer_tracker
            );
        }
        Either::Right(seekable_stream) => {
            let mut read_stream = seekable_stream.lock().await;
            if offset > 0 {
                read_stream.seek(SeekFrom::Start(offset)).await?;
            }

            transfer_fragments!(
                read_stream,
                channel,
                upload_buffer,
                fragment_size,
                last,
                fragments,
                recv,
                chunk_buffer,
                receiver,
                transfer_tracker
            );
        }
    };

    Ok(())
}

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use log::{error, trace, warn};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::select;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, Mutex};
use tokio_util::either::Either;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use commons::error::mdsftp_error::{MDSFTPError, MDSFTPResult};
use protocol::mdsftp::data::{
    ChunkErrorKind, ChunkRange, CommitFlags, LockKind, PutFlags, ReserveFlags,
};
use protocol::mdsftp::handler::{
    AbstractFileStream, AbstractReadStream, AbstractWriteStream, Channel, ChannelPacketHandler,
    DownloadDelegator, UploadDelegator,
};

use crate::file_transfer::transfer_manager::{mdsftp_upload, SizeParams};
use crate::io::fragment_ledger::FragmentLedger;
use crate::locking::file_read_guard::FileReadGuard;
use crate::locking::file_write_guard::FileWriteGuard;
use commons::error::io_error::MeowithIoError;

pub struct MeowithMDSFTPChannelPacketHandler {
    fragment_ledger: FragmentLedger,
    read_guard: Option<Arc<FileReadGuard<Uuid>>>,
    write_guard: Option<Arc<FileWriteGuard<Uuid>>>,
    recv_ack_sender: Option<Arc<Sender<u32>>>,
    chunk_buffer: u16,
    fragment_size: u32,
    reservation_details: Option<ReservationDetails>,
    auto_close: Arc<AtomicBool>,
    receive_file_stream: Option<AbstractWriteStream>,
    upload_file_stream: Option<AbstractReadStream>,
    upload_local_file_stream: Option<AbstractFileStream>,
    upload_cancel: Option<CancellationToken>,
    data_transferred: Arc<AtomicU64>,
}

impl MeowithMDSFTPChannelPacketHandler {
    pub fn new(fragment_ledger: FragmentLedger, chunk_buffer: u16, fragment_size: u32) -> Self {
        MeowithMDSFTPChannelPacketHandler {
            fragment_ledger,
            read_guard: None,
            write_guard: None,
            recv_ack_sender: None,
            chunk_buffer,
            fragment_size,
            reservation_details: None,
            auto_close: Arc::new(AtomicBool::new(true)),
            receive_file_stream: None,
            upload_file_stream: None,
            upload_local_file_stream: None,
            upload_cancel: None,
            data_transferred: Default::default(),
        }
    }
}

impl MeowithMDSFTPChannelPacketHandler {
    async fn start_receiving(&mut self, id: Uuid, append: bool) -> MDSFTPResult<()> {
        self.write_guard = Some(Arc::new(
            self.fragment_ledger
                .lock_table()
                .try_write(id)
                .await
                .map_err(|_| MDSFTPError::ReservationError)?,
        ));
        self.receive_file_stream = Some(
            if append {
                self.fragment_ledger.fragment_append_stream(&id).await
            } else {
                self.fragment_ledger.fragment_write_stream(&id).await
            }
            .map_err(|_| MDSFTPError::RemoteError)?,
        );

        Ok(())
    }

    async fn start_uploading(
        &mut self,
        channel: Channel,
        size: u64,
        chunk_buffer: u16,
        range: Option<ChunkRange>,
    ) -> MDSFTPResult<()> {
        let (tx, rx) = mpsc::channel(self.chunk_buffer as usize + 10usize);
        self.recv_ack_sender = Some(Arc::new(tx));

        let read = self.upload_file_stream.clone();
        let read_file = self.upload_local_file_stream.clone();
        let transferred = self.data_transferred.clone();
        let cancellation_token = CancellationToken::new();
        let fragment_size = self.fragment_size;
        self.upload_cancel = Some(cancellation_token.clone());

        tokio::spawn(async move {
            let stream: Either<AbstractReadStream, AbstractFileStream> = if let Some(read) = read {
                Either::Left(read)
            } else if let Some(read_file) = read_file {
                Either::Right(read_file)
            } else {
                error!("no stream for upload present");
                channel.close(Err(MDSFTPError::Internal)).await;
                return;
            };

            select! {
                upload = mdsftp_upload(&channel, stream, SizeParams {size,range}, rx, chunk_buffer, transferred, fragment_size) => {
                    match upload {
                        Ok(_) => {
                            trace!("mdsftp_upload finished");
                            channel.close(Ok(())).await;
                        }
                        Err(err) => {
                            warn!("File upload mdsftp_error {}", err);
                            channel.close(Err(err)).await;
                        }
                    }
                }
                _ = cancellation_token.cancelled() => {
                    trace!("mdsftp_upload cancelled");
                    channel.close(Ok(())).await;
                }
            }
        });

        Ok(())
    }
}

#[derive(Clone, Debug)]
struct ReservationDetails {
    id: Uuid,
    size: u64,
    #[allow(unused)]
    durable: bool,
}

#[async_trait]
impl ChannelPacketHandler for MeowithMDSFTPChannelPacketHandler {
    async fn handle_file_chunk(
        &mut self,
        channel: Channel,
        chunk: &[u8],
        id: u32,
        is_last: bool,
    ) -> MDSFTPResult<()> {
        match self.receive_file_stream.as_ref() {
            None => {
                return Err(MDSFTPError::ConnectionError);
            }
            Some(stream) => {
                trace!("Receiving MDSFTP chunk {id} {is_last}");
                let mut stream = stream.lock().await;
                if let Err(e) = stream.write_all(chunk).await {
                    drop(stream);
                    channel.close(Err(MDSFTPError::from(e))).await;
                    return Err(MDSFTPError::Internal);
                }
                self.data_transferred
                    .fetch_add(chunk.len() as u64, Ordering::SeqCst);

                trace!("Written MDSFTP chunk to target");

                if is_last {
                    // Drop the stream early so that by the time the handler awaits it,
                    // the writer is closed.
                    if self.auto_close.load(Ordering::Relaxed) {
                        if let Err(e) = stream.shutdown().await {
                            drop(stream);
                            channel.close(Err(MDSFTPError::from(e))).await;
                            return Err(MDSFTPError::Internal);
                        }
                    } else if let Err(e) = stream.flush().await {
                        drop(stream);
                        channel.close(Err(MDSFTPError::from(e))).await;
                        return Err(MDSFTPError::Internal);
                    }
                    drop(stream);
                    self.receive_file_stream = None;

                    if let Some(details) = self.reservation_details.as_ref() {
                        trace!("Releasing reservation {:?}", &details);
                        self.fragment_ledger
                            .release_reservation(&details.id, details.size)
                            .await
                            .map_err(|_| MDSFTPError::ReservationError)?;
                    }
                    channel.respond_receive_ack(id).await?;
                    trace!("MDSFTP chunk ack resp sent");
                    channel.close(Ok(())).await;
                } else {
                    drop(stream);
                    channel.respond_receive_ack(id).await?;
                    trace!("MDSFTP chunk ack resp sent");
                }
            }
        }

        Ok(())
    }

    async fn handle_retrieve(
        &mut self,
        channel: Channel,
        id: Uuid,
        chunk_buffer: u16,
        range: Option<ChunkRange>,
    ) -> MDSFTPResult<()> {
        trace!("handle_retrieve {id} {chunk_buffer}");
        let meta = self.fragment_ledger.fragment_meta(&id).await;
        if meta.is_none() {
            trace!("No such chunk id");
            return Err(MDSFTPError::NoSuchChunkId);
        }
        let meta = meta.unwrap();
        let size: u64 = meta.disk_content_size;

        self.read_guard = Some(Arc::new(
            self.fragment_ledger
                .lock_table()
                .try_read(id)
                .await
                .map_err(|_| MDSFTPError::ReservationError)?,
        ));
        self.upload_local_file_stream = Some(
            self.fragment_ledger
                .fragment_read_omni_stream(&id)
                .await
                .map_err(|_| MDSFTPError::RemoteError)?,
        );

        self.start_uploading(channel, size, chunk_buffer, range)
            .await?;
        Ok(())
    }

    async fn handle_put(
        &mut self,
        channel: Channel,
        flags: PutFlags,
        chunk_id: Uuid,
        content_size: u64,
    ) -> MDSFTPResult<()> {
        match self.fragment_ledger.resume_reservation(&chunk_id).await {
            Ok(reservation) => {
                if content_size == reservation.file_space - reservation.completed {
                    self.reservation_details = Some(ReservationDetails {
                        id: chunk_id,
                        size: reservation.file_space,
                        durable: reservation.durable,
                    });
                    self.start_receiving(chunk_id, flags.append).await?;
                    channel.respond_put_ok(self.chunk_buffer).await?;
                } else {
                    trace!("ChunkErrorKind::NotAvailable invalid content size");
                    channel
                        .respond_put_err(ChunkErrorKind::NotAvailable)
                        .await?;
                    channel.close(Ok(())).await;
                }
            }
            Err(MeowithIoError::NotFound) => {
                channel.respond_put_err(ChunkErrorKind::NotFound).await?;
                channel.close(Ok(())).await;
            }
            Err(_) => {
                trace!("ChunkErrorKind::NotAvailable invalid reservation");
                channel
                    .respond_put_err(ChunkErrorKind::NotAvailable)
                    .await?;
                channel.close(Ok(())).await;
            }
        }

        Ok(())
    }

    async fn handle_reserve(
        &mut self,
        channel: Channel,
        desired_size: u64,
        flags: ReserveFlags,
    ) -> MDSFTPResult<()> {
        match self
            .fragment_ledger
            .try_reserve(desired_size, flags.durable)
            .await
        {
            Ok(id) => {
                self.reservation_details = Some(ReservationDetails {
                    id,
                    size: desired_size,
                    durable: flags.durable,
                });

                if flags.auto_start {
                    // The only case is when an upload is non-durable which would be non-append anyway.
                    self.start_receiving(id, false).await?;
                }
                channel.respond_reserve_ok(id, self.chunk_buffer).await?;
                if flags.temp {
                    if let Err(e) = self.fragment_ledger.pause_reservation(&id).await {
                        error!("Unexpected internal mdsftp_error occurred {}", e);
                        channel.close(Err(MDSFTPError::Internal)).await;
                    } else {
                        channel.close(Ok(())).await;
                    }
                }
            }
            Err(_) => {
                channel
                    .respond_reserve_err(self.fragment_ledger.get_available_space())
                    .await?;
                channel.close(Ok(())).await;
            }
        }
        Ok(())
    }

    async fn handle_lock_req(
        &mut self,
        channel: Channel,
        chunk_id: Uuid,
        kind: LockKind,
    ) -> MDSFTPResult<()> {
        if !self.fragment_ledger.fragment_exists(&chunk_id).await {
            channel
                .respond_lock_err(chunk_id, kind, ChunkErrorKind::NotFound)
                .await?;
            channel.close(Ok(())).await;
            return Ok(());
        }

        match kind {
            LockKind::Read => match self.fragment_ledger.lock_table().try_read(chunk_id).await {
                Ok(guard) => {
                    self.read_guard = Some(Arc::new(guard));
                }
                Err(_) => {
                    channel
                        .respond_lock_err(chunk_id, kind, ChunkErrorKind::NotAvailable)
                        .await?;
                }
            },
            LockKind::Write => match self.fragment_ledger.lock_table().try_write(chunk_id).await {
                Ok(guard) => {
                    self.write_guard = Some(Arc::new(guard));
                }
                Err(_) => {
                    channel
                        .respond_lock_err(chunk_id, kind, ChunkErrorKind::NotAvailable)
                        .await?;
                }
            },
        };

        Ok(())
    }

    async fn handle_receive_ack(&mut self, _channel: Channel, chunk_id: u32) -> MDSFTPResult<()> {
        if let Some(tx) = self.recv_ack_sender.as_ref() {
            trace!("handle_receive_ack {chunk_id}");
            let a = tx.send(chunk_id).await;
            if let Err(a) = a {
                error!("{a}");
            }
            Ok(())
        } else {
            trace!("handle_receive_ack no tx");
            Err(MDSFTPError::ConnectionError)
        }
    }

    async fn handle_reserve_cancel(
        &mut self,
        channel: Channel,
        chunk_id: Uuid,
    ) -> MDSFTPResult<()> {
        let _ = self.fragment_ledger.cancel_reservation(&chunk_id).await;
        channel.close(Ok(())).await;
        Ok(())
    }

    async fn handle_delete_chunk(&mut self, channel: Channel, chunk_id: Uuid) -> MDSFTPResult<()> {
        let _ = self.fragment_ledger.delete_chunk(&chunk_id).await;
        channel.close(Ok(())).await;
        Ok(())
    }

    async fn handle_commit(
        &mut self,
        channel: Channel,
        chunk_id: Uuid,
        flags: CommitFlags,
    ) -> MDSFTPResult<()> {
        if flags.r#final {
            let _ = self.fragment_ledger.commit_chunk(&chunk_id).await;
        } else if flags.keep_alive {
            let _ = self.fragment_ledger.commit_alive(&chunk_id).await;
        } else if flags.reject {
            let _ = self.fragment_ledger.delete_chunk(&chunk_id).await;
        }

        channel.close(Ok(())).await;
        Ok(())
    }

    async fn handle_query(&mut self, channel: Channel, chunk_id: Uuid) -> MDSFTPResult<()> {
        if let Some(data) = self.fragment_ledger.fragment_meta_ex(&chunk_id).await {
            channel.respond_query(data.disk_content_size, true).await?
        } else {
            channel.respond_query(0, false).await?
        }
        channel.close(Ok(())).await;
        Ok(())
    }

    async fn handle_interrupt(&mut self) -> MDSFTPResult<()> {
        trace!("handle_interrupt called {:?}", self.reservation_details);

        if let Some(token) = self.upload_cancel.as_ref() {
            trace!("Cancelling upload, interrupted");
            token.cancel();
        }
        match self.receive_file_stream.as_ref() {
            None => {}
            Some(stream) => {
                {
                    trace!("Cancelling upload, interrupted");
                    // Ignoring auto-close as the stream will be killed anyway.
                    let mut stream = stream.lock().await;
                    let _ = stream.shutdown().await;
                }
                self.receive_file_stream = None
            }
        }
        self.upload_file_stream = None;

        if let Some(details) = self.reservation_details.as_ref() {
            self.fragment_ledger
                .release_reservation(&details.id, self.data_transferred.load(Ordering::SeqCst))
                .await
                .map_err(|_| MDSFTPError::ReservationError)?;
        }

        Ok(())
    }
}

#[async_trait]
impl UploadDelegator for MeowithMDSFTPChannelPacketHandler {
    async fn delegate_upload(
        &mut self,
        channel: Channel,
        source: AbstractReadStream,
        size: u64,
        chunk_buffer: u16,
    ) -> MDSFTPResult<()> {
        self.upload_file_stream = Some(source);
        self.start_uploading(channel, size, chunk_buffer, None)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl DownloadDelegator for MeowithMDSFTPChannelPacketHandler {
    async fn delegate_download(
        &mut self,
        _channel: Channel,
        output: Arc<Mutex<Pin<Box<dyn AsyncWrite + Unpin + Send>>>>,
        auto_close: bool,
    ) -> MDSFTPResult<()> {
        self.receive_file_stream = Some(output);
        self.auto_close.store(auto_close, Ordering::SeqCst);
        Ok(())
    }
}

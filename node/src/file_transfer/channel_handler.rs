use async_trait::async_trait;
use log::{debug, warn};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, BufWriter};
use tokio::select;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use protocol::mdsftp::data::{ChunkErrorKind, LockKind, ReserveFlags};
use protocol::mdsftp::error::{MDSFTPError, MDSFTPResult};
use protocol::mdsftp::handler::{
    Channel, ChannelPacketHandler, DownloadDelegator, UploadDelegator,
};

use crate::file_transfer::transfer_manager::mdsftp_upload;
use crate::io::fragment_ledger::FragmentLedger;
use crate::locking::file_read_guard::FileReadGuard;
use crate::locking::file_write_guard::FileWriteGuard;

pub type AbstractReadStream = Arc<Mutex<BufReader<Pin<Box<dyn AsyncRead + Unpin + Send>>>>>;
pub type AbstractWriteStream = Arc<Mutex<BufWriter<Pin<Box<dyn AsyncWrite + Unpin + Send>>>>>;

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
            upload_cancel: None,
            data_transferred: Default::default(),
        }
    }
}

impl MeowithMDSFTPChannelPacketHandler {
    async fn start_receiving(&mut self, id: Uuid) -> MDSFTPResult<()> {
        self.write_guard = Some(Arc::new(
            self.fragment_ledger
                .lock_table()
                .try_write(id)
                .await
                .map_err(|_| MDSFTPError::ReservationError)?,
        ));
        self.receive_file_stream = Some(Arc::new(Mutex::new(BufWriter::new(Box::pin(
            self.fragment_ledger
                .fragment_write_stream(&id)
                .await
                .map_err(|_| MDSFTPError::RemoteError)?,
        )))));

        Ok(())
    }

    async fn start_uploading(
        &mut self,
        channel: Channel,
        size: u64,
        chunk_buffer: u16,
    ) -> MDSFTPResult<()> {
        let (tx, rx) = mpsc::channel(self.chunk_buffer as usize + 10usize);
        self.recv_ack_sender = Some(Arc::new(tx));

        let read = self.upload_file_stream.clone();
        let transferred = self.data_transferred.clone();
        let cancellation_token = CancellationToken::new();
        let fragment_size = self.fragment_size;
        self.upload_cancel = Some(cancellation_token.clone());

        tokio::spawn(async move {
            let read = read.unwrap();
            let read = read.lock().await;
            select! {
                upload = mdsftp_upload(&channel, read, size, rx, chunk_buffer, transferred, fragment_size) => {
                    match upload {
                        Ok(_) => {}
                        Err(err) => {
                            warn!("File upload error {}", err);
                        }
                    }
                }
                _ = cancellation_token.cancelled() => {}
            }

            channel.close(Ok(())).await;
        });

        Ok(())
    }
}

#[allow(unused)]
#[derive(Clone, Debug)]
struct ReservationDetails {
    id: Uuid,
    size: u64,
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
                let mut stream = stream.lock().await;
                if let Err(e) = stream.write_all(chunk).await {
                    channel.close(Err(MDSFTPError::from(e))).await;
                    return Err(MDSFTPError::Internal);
                }
                self.data_transferred
                    .fetch_and(chunk.len() as u64, Ordering::SeqCst);

                if is_last {
                    // Drop the stream early so that by the time the handler awaits it,
                    // the writer is closed.
                    if self.auto_close.load(Ordering::Relaxed) {
                        if let Err(e) = stream.shutdown().await {
                            channel.close(Err(MDSFTPError::from(e))).await;
                            return Err(MDSFTPError::Internal);
                        }
                        drop(stream);
                        self.receive_file_stream = None;
                    }

                    if let Some(details) = self.reservation_details.as_ref() {
                        debug!("Releasing reservation {:?}", &details);
                        self.fragment_ledger
                            .release_reservation(&details.id, details.size)
                            .await
                            .map_err(|_| MDSFTPError::ReservationError)?;
                    }
                    channel.respond_receive_ack(id).await?;
                    channel.close(Ok(())).await;
                } else {
                    channel.respond_receive_ack(id).await?;
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
    ) -> MDSFTPResult<()> {
        let meta = self.fragment_ledger.fragment_meta(&id).await;
        if meta.is_none() {
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
        self.upload_file_stream = Some(Arc::new(Mutex::new(BufReader::new(Box::pin(
            self.fragment_ledger
                .fragment_read_stream(&id)
                .await
                .map_err(|_| MDSFTPError::RemoteError)?,
        )))));

        self.start_uploading(channel, size, chunk_buffer).await?;
        Ok(())
    }

    async fn handle_put(
        &mut self,
        _channel: Channel,
        chunk_id: Uuid,
        _content_size: u64,
    ) -> MDSFTPResult<()> {
        self.start_receiving(chunk_id).await?;

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
                    self.start_receiving(id).await?;
                }
                channel.respond_reserve_ok(id, self.chunk_buffer).await?;
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

        return Ok(());
    }

    async fn handle_receive_ack(&mut self, _channel: Channel, chunk_id: u32) -> MDSFTPResult<()> {
        if let Some(tx) = self.recv_ack_sender.as_ref() {
            let _ = tx.send(chunk_id).await;
            Ok(())
        } else {
            Err(MDSFTPError::ConnectionError)
        }
    }

    async fn handle_interrupt(&mut self) -> MDSFTPResult<()> {
        if let Some(details) = self.reservation_details.as_ref() {
            self.fragment_ledger
                .release_reservation(&details.id, self.data_transferred.load(Ordering::SeqCst))
                .await
                .map_err(|_| MDSFTPError::ReservationError)?;
        }

        if let Some(token) = self.upload_cancel.as_ref() {
            token.cancel();
        }
        match self.receive_file_stream.as_ref() {
            None => {}
            Some(stream) => {
                {
                    // Ignoring auto-close as the stream will be killed anyways.
                    let mut stream = stream.lock().await;
                    let _ = stream.shutdown().await;
                }
                self.receive_file_stream = None
            }
        }
        self.upload_file_stream = None;

        Ok(())
    }
}

#[async_trait]
impl<T> UploadDelegator<T> for MeowithMDSFTPChannelPacketHandler
where
    T: AsyncRead + Unpin + Send + 'static,
{
    async fn delegate_upload(
        &mut self,
        channel: Channel,
        source: T,
        size: u64,
        chunk_buffer: u16,
    ) -> MDSFTPResult<()> {
        self.upload_file_stream = Some(Arc::new(Mutex::new(BufReader::new(Box::pin(source)))));
        self.start_uploading(channel, size, chunk_buffer).await?;
        Ok(())
    }
}

#[async_trait]
impl<T> DownloadDelegator<T> for MeowithMDSFTPChannelPacketHandler
where
    T: AsyncWrite + Unpin + Send + 'static,
{
    async fn delegate_download(&mut self, _channel: Channel, output: T, auto_close: bool) -> MDSFTPResult<()> {
        self.receive_file_stream = Some(Arc::new(Mutex::new(BufWriter::new(Box::pin(output)))));
        self.auto_close.store(auto_close, Ordering::SeqCst);
        Ok(())
    }
}

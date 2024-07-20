use std::io;
use std::io::ErrorKind;
use std::pin::Pin;
use std::task::{Context, Poll};

use actix_web::web;
use actix_web::web::Bytes;
use futures_util::StreamExt;
use tokio::io::{AsyncRead, ReadBuf};

struct UploadStream {
    pub payload: web::Payload,
    buffer: Option<Bytes>,
}

impl AsyncRead for UploadStream {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        // If the previous chunk was not read completely, try finishing reading.
        if let Some(ref mut remaining) = self.buffer {
            let to_copy = std::cmp::min(remaining.len(), buf.remaining());
            buf.put_slice(&remaining.split_to(to_copy));

            if !remaining.is_empty() {
                return Poll::Ready(Ok(()));
            }

            self.buffer.take();
            return Poll::Ready(Ok(()));
        }

        match self.payload.poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                let to_copy = std::cmp::min(chunk.len(), buf.remaining());
                buf.put_slice(&chunk[..to_copy]);

                // Couldn't write all data at once, store in buffer to read later
                if to_copy < chunk.len() {
                    self.buffer = Some(chunk.slice(to_copy..));
                }

                Poll::Ready(Ok(()))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Err(io::Error::new(ErrorKind::Other, e))),
            Poll::Ready(None) => Poll::Ready(Ok(())),
            Poll::Pending => Poll::Pending
        }
    }
}
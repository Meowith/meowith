use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex;
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;
use uuid::{Bytes, Uuid};
use crate::framework::error::ProtocolError;
use crate::framework::server::{Protocol, ProtocolConnection};

/// Handles incoming connections, including TLS handshake, authentication, and protocol handoff.
pub async fn handle_incoming_connection(
    acceptor: &TlsAcceptor,
    listener: &TcpListener,
    shutdown_sender: &Arc<Mutex<Option<Sender<()>>>>,
    running: &Arc<AtomicBool>,
    protocol_handler: Arc<dyn Protocol>
) -> Result<(), ProtocolError> {
    let stream = select_tcp_connection(listener, shutdown_sender, running).await?;
    let mut tls_stream = accept_tls_connection(acceptor.clone(), stream).await?;
    let id = authenticate_connection(&mut tls_stream).await?;

    let connection = ProtocolConnection { stream: tls_stream, id };
    protocol_handler.handle_connection(connection).await?;

    Ok(())
}

/// Waits for either a TCP connection or a shutdown signal.
async fn select_tcp_connection(
    listener: &TcpListener,
    shutdown_sender: &Arc<Mutex<Option<Sender<()>>>>,
    running: &Arc<AtomicBool>
) -> Result<TcpStream, ProtocolError> {
    let mut shutdown_rx = {
        let lock = shutdown_sender.lock().await;
        lock.as_ref().unwrap().subscribe()
    };

    if !running.load(Ordering::Relaxed) {
        return Err(ProtocolError::ShuttingDown);
    }

    tokio::select! {
            _ = shutdown_rx.recv() => Err(ProtocolError::ShuttingDown),
            result = listener.accept() => result.map(|(stream, _)| stream).map_err(|_| ProtocolError::ConnectionError),
        }
}

/// Accepts an incoming TLS connection using the provided acceptor.
async fn accept_tls_connection(
    acceptor: TlsAcceptor,
    stream: TcpStream
) -> Result<TlsStream<TcpStream>, ProtocolError> {
    acceptor.accept(stream).await.map_err(|_| ProtocolError::ConnectionError)
}

/// Authenticates the incoming connection by reading a UUID from the stream.
async fn authenticate_connection(
    stream: &mut TlsStream<TcpStream>
) -> Result<Uuid, ProtocolError> {
    let mut auth_header = [0u8; 16];

    stream.read_exact(&mut auth_header).await.map_err(|_| ProtocolError::AuthenticationFailed)?;

    let id = Uuid::from_bytes(Bytes::try_from(auth_header).unwrap_or_else(|_| Uuid::nil().to_bytes_le()));
    Ok(id)
}
use crate::framework::auth::ProtocolAuthenticator;
use crate::framework::connection::ProtocolConnection;
use crate::framework::error::ProtocolError;
use crate::framework::server::Protocol;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex;
use tokio_rustls::TlsStream;
use tokio_rustls::TlsAcceptor;
use crate::framework::packet::parser::{Packet, PacketBuilder, PacketParser};

#[derive(Clone)]
pub struct ProtocolBehaviour<T: Packet + 'static + Send, A: Send + 'static> {
    pub protocol_handler: Arc<dyn Protocol<T>>,
    pub packet_parser: Arc<dyn PacketParser<T>>,
    pub packet_builder: Arc<dyn PacketBuilder<T>>,
    pub authenticator: Arc<dyn ProtocolAuthenticator<A>>,
}

/// Handles incoming connections, including TLS handshake, authentication, and protocol handoff.
pub async fn handle_incoming_connection<T: Packet + 'static + Send, A: Send + 'static>(
    acceptor: &TlsAcceptor,
    listener: &TcpListener,
    shutdown_sender: &Arc<Mutex<Option<Sender<()>>>>,
    running: &Arc<AtomicBool>,
    protocol_behaviour: ProtocolBehaviour<T, A>,
    connections: Arc<Mutex<Vec<ProtocolConnection<T>>>>,
) -> Result<(), ProtocolError> {
    let stream = select_tcp_connection(listener, shutdown_sender, running).await?;
    let mut tls_stream = accept_tls_connection(acceptor.clone(), stream).await?;
    let authenticated = protocol_behaviour
        .authenticator
        .authenticate(&mut tls_stream)
        .await;

    if authenticated.is_err() {
        return Err(ProtocolError::AuthenticationFailed);
    }

    let connection = ProtocolConnection::new(tls_stream, protocol_behaviour.packet_parser, protocol_behaviour.packet_builder)
        .await
        .map_err(|_| ProtocolError::ConnectionError)?;
    protocol_behaviour
        .protocol_handler
        .handle_connection(&connection)
        .await?;
    connections.lock().await.push(connection);

    Ok(())
}

/// Waits for either a TCP connection or a shutdown signal.
async fn select_tcp_connection(
    listener: &TcpListener,
    shutdown_sender: &Arc<Mutex<Option<Sender<()>>>>,
    running: &Arc<AtomicBool>,
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
    stream: TcpStream,
) -> Result<TlsStream<TcpStream>, ProtocolError> {
    acceptor
        .accept(stream)
        .await
        .map(TlsStream::from)
        .map_err(|_| ProtocolError::ConnectionError)
}

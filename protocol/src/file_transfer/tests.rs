#[cfg(test)]
mod tests {
    use crate::file_transfer::authenticator::ConnectionAuthContext;
    use crate::file_transfer::channel::MDSFTPChannel;
    use crate::file_transfer::data::{LockAcquireResult, LockKind};
    use crate::file_transfer::error::MDSFTPResult;
    use crate::file_transfer::handler::{Channel, ChannelPacketHandler, PacketHandler};
    use crate::file_transfer::pool::{MDSFTPPool, PacketHandlerRef};
    use crate::file_transfer::server::MDSFTPServer;
    use async_trait::async_trait;
    use commons::autoconfigure::ssl_conf::{gen_test_ca, gen_test_certs};
    use commons::context::microservice_request_context::NodeAddrMap;
    use log::{debug, info};
    use logging::initialize_test_logging;
    use openssl::x509::X509VerifyResult;
    use std::collections::HashMap;
    use std::io::Write;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::{Mutex, RwLock};
    use tokio::time::sleep;
    use uuid::Uuid;

    struct HandlerStats {
        pub channels_opened: u32,
        pub channels_closed: u32,
    }

    impl HandlerStats {
        fn default() -> Self {
            HandlerStats {
                channels_opened: 0,
                channels_closed: 0,
            }
        }
    }

    struct TestIncomingHandler {
        stats: Arc<Mutex<HandlerStats>>,
        received: Option<Arc<Mutex<Vec<u8>>>>,
        name: String,
    }

    impl TestIncomingHandler {
        fn default(
            stats: Arc<Mutex<HandlerStats>>,
            received: Option<Arc<Mutex<Vec<u8>>>>,
            name: String,
        ) -> Self {
            TestIncomingHandler {
                stats,
                received,
                name,
            }
        }
    }

    #[async_trait]
    impl PacketHandler for TestIncomingHandler {
        async fn channel_incoming(&mut self, channel: MDSFTPChannel, conn_id: Uuid) {
            info!("{} Channel open {conn_id}", &self.name);
            self.stats.lock().await.channels_opened += 1;
            let await_handler = channel
                .set_incoming_handler(Box::new(EchoChannel {
                    store_buf: self.received.clone(),
                }))
                .await;
            debug!("Handler registered");
            tokio::spawn(async move {
                let _no_drop = channel;
                await_handler.await;
            });
        }

        async fn channel_close(&mut self, _channel_id: u32, _conn_id: Uuid) {
            info!("{} Channel close", &self.name);
            self.stats.lock().await.channels_closed += 1;
        }

        async fn channel_err(&mut self, _channel_id: u32, _conn_id: Uuid) {
            info!("{} Channel err", &self.name)
        }
    }

    struct EchoChannel {
        store_buf: Option<Arc<Mutex<Vec<u8>>>>,
    }

    #[async_trait]
    impl ChannelPacketHandler for EchoChannel {
        async fn handle_file_chunk(
            &mut self,
            channel: Channel,
            chunk: &[u8],
            id: u32,
            is_last: bool,
        ) -> MDSFTPResult<()> {
            match self.store_buf.as_ref() {
                None => {
                    channel
                        .respond_chunk(is_last, id, chunk)
                        .await
                        .expect("Chunk echo failed");
                    if is_last {
                        channel.close().await;
                    }
                    Ok(())
                }
                Some(buf) => {
                    let mut buf = buf.lock().await;
                    buf.write(chunk).expect("write fail");
                    if is_last {
                        channel.close().await;
                    }
                    Ok(())
                }
            }
        }

        async fn handle_retrieve(
            &mut self,
            channel: Channel,
            _chunk_id: Uuid,
            _chunk_buffer: u16,
        ) -> MDSFTPResult<()> {
            channel.close().await;
            Ok(())
        }

        async fn handle_put(
            &mut self,
            channel: Channel,
            _chunk_id: Uuid,
            _content_size: u64,
        ) -> MDSFTPResult<()> {
            channel.close().await;
            Ok(())
        }

        async fn handle_reserve(
            &mut self,
            channel: Channel,
            _desired_size: u64,
            _auto_start: bool,
        ) -> MDSFTPResult<()> {
            channel
                .respond_reserve_ok(Uuid::new_v4(), 16)
                .await
                .expect("ReserveOk respond failed");
            channel.close().await;
            Ok(())
        }

        async fn handle_lock_req(
            &mut self,
            channel: Channel,
            chunk_id: Uuid,
            kind: LockKind,
        ) -> MDSFTPResult<()> {
            channel
                .respond_lock_ok(chunk_id, kind)
                .await
                .expect("LockOk respond failed");
            channel.close().await;
            Ok(())
        }

        async fn handle_receive_ack(
            &mut self,
            _channel: Channel,
            _chunk_id: u32,
        ) -> MDSFTPResult<()> {
            Ok(())
        }

        async fn handle_interrupt(&self) -> MDSFTPResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    #[ntest::timeout(3000)]
    async fn test() {
        initialize_test_logging();

        let (ca, ca_key) = gen_test_ca();
        let (cert, key) = gen_test_certs(&ca, &ca_key);

        assert_eq!(ca.issued(&cert), X509VerifyResult::OK);

        let id1 = Uuid::new_v4();

        let mut conn_map = HashMap::new();
        conn_map.insert(id1, "127.0.0.1".to_string());
        let conn_map: NodeAddrMap = Arc::new(RwLock::new(conn_map));

        let connection_auth_context = Arc::new(ConnectionAuthContext {
            root_certificate: ca.clone(),
            authenticator: None,
            port: 7670,
        });

        let server_stats = Arc::new(Mutex::new(HandlerStats::default()));
        let server_handler: PacketHandlerRef = Arc::new(Mutex::new(Box::new(
            TestIncomingHandler::default(server_stats.clone(), None, "server_pool".to_string()),
        )));

        let mut server = MDSFTPServer::new(
            connection_auth_context.clone(),
            conn_map.clone(),
            server_handler,
        )
        .await;
        assert!(server.start(&cert, &key).await.is_ok());

        let client_stats = Arc::new(Mutex::new(HandlerStats::default()));
        let client_received = Arc::new(Mutex::new(Vec::<u8>::new()));
        let client_handler: PacketHandlerRef = Arc::new(Mutex::new(Box::new(
            TestIncomingHandler::default(client_stats.clone(), None, "client_pool".to_string()),
        )));
        let mut client_pool = MDSFTPPool::new(connection_auth_context, conn_map);
        client_pool.set_packet_handler(client_handler).await;

        {
            let channel = client_pool.channel(&id1).await.unwrap();
            let await_handler = channel
                .set_incoming_handler(Box::new(EchoChannel {
                    store_buf: Some(client_received.clone()),
                }))
                .await;
            let send = channel.send_chunk(false, 0, &[0u8, 1u8, 2u8]).await;
            assert!(send.is_ok());
            let send = channel.send_chunk(true, 0, &[3u8, 4u8, 5u8]).await;
            assert!(send.is_ok());

            await_handler.await;
        }

        sleep(Duration::from_millis(100)).await;
        assert_eq!(client_stats.lock().await.channels_opened, 0);

        assert_eq!(server_stats.lock().await.channels_opened, 1);

        assert_eq!(
            client_received.lock().await.clone(),
            vec![0u8, 1u8, 2u8, 3u8, 4u8, 5u8]
        );

        {
            debug!("Test lock acquire");
            let channel = client_pool.channel(&id1).await.unwrap();
            let id = Uuid::new_v4();
            let lock_req = channel.request_lock(LockKind::Read, id).await;
            assert!(lock_req.is_ok());
            assert_eq!(
                lock_req.unwrap(),
                LockAcquireResult {
                    kind: LockKind::Read,
                    chunk_id: id,
                }
            );
        }

        {
            debug!("Test reserve");
            let channel = client_pool.channel(&id1).await.unwrap();
            let lock_req = channel.try_reserve(15, true).await;
            assert!(lock_req.is_ok());
        }

        client_pool.shutdown().await;
        server.shutdown().await;
    }
}

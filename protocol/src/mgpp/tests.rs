#[cfg(test)]
mod int_tests {
    use std::net::{IpAddr, SocketAddr};
    use std::str::FromStr;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use openssl::x509::X509VerifyResult;
    use tokio::sync::Mutex;
    use tokio::time::sleep;
    use uuid::Uuid;

    use crate::framework::auth::ConnectionAuthContext;
    use crate::mgpp::client::MGPPClient;
    use crate::mgpp::handler::{InvalidateCacheHandler, MGPPHandlers};
    use crate::mgpp::packet::MGPPPacket;
    use crate::mgpp::server::MGPPServer;
    use commons::autoconfigure::ssl_conf::{gen_test_ca, gen_test_certs};
    use commons::pause_handle::ApplicationPauseHandle;
    use logging::initialize_test_logging;

    const CACHE_ID: usize = 5;
    const FIRST_BYTE: u8 = 32;

    #[async_trait]
    impl InvalidateCacheHandler for TestCacheHandler {
        async fn handle_invalidate(&self, cache_id: u32, cache: &[u8]) {
            if CACHE_ID == cache_id as usize && cache[0] == FIRST_BYTE {
                self.received.lock().await.store(true, Ordering::SeqCst);
            }
        }
    }

    #[derive(Debug)]
    struct TestCacheHandler {
        pub received: Arc<Mutex<AtomicBool>>,
    }

    struct DummyPauseHandle {
        pub pause_count: Arc<Mutex<AtomicUsize>>,
        pub resume_count: Arc<Mutex<AtomicUsize>>,
    }

    #[async_trait]
    impl ApplicationPauseHandle for DummyPauseHandle {
        async fn pause(&self) {
            self.pause_count.lock().await.fetch_add(1, Ordering::SeqCst);
        }

        async fn resume(&self) {
            self.resume_count
                .lock()
                .await
                .fetch_add(1, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    #[ntest::timeout(20000)]
    async fn test() {
        initialize_test_logging();

        let (ca, ca_key) = gen_test_ca();
        let (cert, key) = gen_test_certs(&ca, &ca_key);

        assert_eq!(ca.issued(&cert), X509VerifyResult::OK);

        let connection_auth_context = Arc::new(ConnectionAuthContext {
            root_certificate: ca.clone(),
            authenticator: None,
            port: 7810,
            own_id: Uuid::new_v4(),
        });

        let server = MGPPServer::new(connection_auth_context.clone());
        assert!(server
            .start_server(7810, (cert.clone(), key.clone()))
            .await
            .is_ok());

        let id = Uuid::new_v4();
        let received = Arc::new(Mutex::new(AtomicBool::new(false)));

        let client = MGPPClient::connect(
            SocketAddr::new(IpAddr::from_str("127.0.0.1").unwrap(), 7810),
            ca.clone(),
            id,
            None,
            MGPPHandlers::new(Box::new(TestCacheHandler {
                received: received.clone(),
            })),
        )
        .await;
        assert!(client.is_ok());
        let client = client.unwrap();
        let dummy_pause_handle: Arc<Box<dyn ApplicationPauseHandle>> =
            Arc::new(Box::new(DummyPauseHandle {
                pause_count: Default::default(),
                resume_count: Default::default(),
            }));
        client
            .set_up_auto_reconnect(dummy_pause_handle.clone())
            .await;

        assert!(client
            .write_packet(MGPPPacket::InvalidateCache {
                cache_id: 5,
                cache_key: vec![32],
            })
            .await
            .is_ok());

        sleep(Duration::from_millis(100)).await;

        {
            let handler = received.lock().await;
            assert!(handler.load(Ordering::SeqCst));
            handler.store(false, Ordering::SeqCst);
        }

        server.shutdown().await;
        drop(server);
        sleep(Duration::from_millis(500)).await;

        // let server = MGPPServer::new(connection_auth_context.clone());
        // assert!(server.start_server(7810, (cert, key)).await.is_ok());

        sleep(Duration::from_millis(2000)).await;

        assert!(client
            .write_packet(MGPPPacket::InvalidateCache {
                cache_id: 5,
                cache_key: vec![32],
            })
            .await
            .is_ok());
    }
}

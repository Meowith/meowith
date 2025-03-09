#[cfg(test)]
mod int_tests {
    use std::net::{IpAddr, SocketAddr};
    use std::str::FromStr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use openssl::x509::X509VerifyResult;
    use tokio::sync::Mutex;
    use tokio::time::sleep;
    use uuid::Uuid;

    use commons::autoconfigure::ssl_conf::{gen_test_ca, gen_test_certs};
    use logging::initialize_test_logging;

    use crate::mdsftp::authenticator::ConnectionAuthContext;
    use crate::mgpp::client::MGPPClient;
    use crate::mgpp::handler::{InvalidateCacheHandler, MGPPHandlers};
    use crate::mgpp::packet::MGPPPacket;
    use crate::mgpp::server::MGPPServer;

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

    #[tokio::test]
    #[ntest::timeout(10000)]
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

        assert!(server.start_server(7810, (cert, key)).await.is_ok());

        {
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

            assert!(client
                .write_packet(MGPPPacket::InvalidateCache {
                    cache_id: 5,
                    cache_key: vec![32],
                })
                .await
                .is_ok());

            sleep(Duration::from_millis(100)).await;

            let handler = received.lock().await;

            assert!(handler.load(Ordering::SeqCst));
        }
    }
}

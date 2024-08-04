#[cfg(test)]
mod tests {
    use std::any::Any;
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

    use crate::catche::catche_client::CatcheClient;
    use crate::catche::catche_server::CatcheServer;
    use crate::catche::handler::CatcheHandler;
    use crate::catche::reader::CatchePacketHandler;
    use crate::mdsftp::authenticator::ConnectionAuthContext;

    const CACHE_ID: usize = 5;

    #[async_trait]
    impl CatcheHandler for TestCatcheHandler {
        async fn handle_invalidate(&mut self, cache_id: u32, _cache: String) {
            if CACHE_ID == cache_id as usize {
                self.received.store(true, Ordering::SeqCst);
            }
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[derive(Debug)]
    struct TestCatcheHandler {
        pub received: AtomicBool,
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

        let mut server = CatcheServer::new(connection_auth_context.clone());

        assert!(server.start_server(7810, (cert, key)).await.is_ok());

        {
            let id = Uuid::new_v4();
            let handler: CatchePacketHandler = Arc::new(Mutex::new(Box::new(TestCatcheHandler {
                received: AtomicBool::new(false),
            })
                as Box<dyn CatcheHandler>));

            let client = CatcheClient::connect(
                &SocketAddr::new(IpAddr::from_str("127.0.0.1").unwrap(), 7810),
                id,
                ca.clone(),
                handler.clone(),
                None,
            )
            .await;
            assert!(client.is_ok());

            let client = client.unwrap();

            assert!(client
                .write_invalidate_packet(5, "test".to_string())
                .await
                .is_ok());

            sleep(Duration::from_millis(100)).await;

            let lock = handler.lock().await;
            let handler = lock.as_any().downcast_ref::<TestCatcheHandler>().unwrap();

            assert!(handler.received.load(Ordering::SeqCst));
        }
    }
}

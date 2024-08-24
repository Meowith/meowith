#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::net::IpAddr;
    use std::path::PathBuf;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::time::Instant;
    use std::{env, fs, io};

    use log::debug;
    use ntest::timeout;
    use openssl::x509::X509VerifyResult;
    use rand::RngCore;
    use tokio::fs::File;
    use tokio::io::{AsyncWriteExt, BufWriter};
    use tokio::sync::{Mutex, RwLock};
    use uuid::Uuid;

    use commons::autoconfigure::ssl_conf::{gen_test_ca, gen_test_certs};
    use commons::context::microservice_request_context::NodeAddrMap;
    use logging::initialize_test_logging;
    use protocol::mdsftp::authenticator::ConnectionAuthContext;
    use protocol::mdsftp::data::{CommitFlags, ReserveFlags};
    use protocol::mdsftp::pool::{MDSFTPPool, PacketHandlerRef};
    use protocol::mdsftp::server::MDSFTPServer;

    use crate::file_transfer::channel_handler::MeowithMDSFTPChannelPacketHandler;
    use crate::file_transfer::packet_handler::MeowithMDSFTPPacketHandler;
    use crate::io::fragment_ledger::FragmentLedger;
    use crate::locking::file_lock_table::FileLockTable;

    struct Cleanup {
        temp_dir: PathBuf,
    }

    fn remove_dir_iteratively(path: &PathBuf) -> io::Result<()> {
        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = remove_dir_iteratively(&self.temp_dir).unwrap();
        }
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_file_transfer() {
        initialize_test_logging();
        let mut temp_dir = env::temp_dir();
        let name = format!("meowith-{}", Uuid::new_v4());
        debug!("Creating temp dir {}", name);
        temp_dir.push(name);
        fs::create_dir_all(&temp_dir).expect("Failed to make temp dir");

        let mut node_dir_one = temp_dir.clone();
        node_dir_one.push("node1");
        fs::create_dir_all(&node_dir_one).expect("Failed to make dir for first node");

        let mut node_dir_two = temp_dir.clone();
        node_dir_two.push("node2");
        fs::create_dir_all(&node_dir_two).expect("Failed to make dir for second node");

        let _cleanup = Cleanup { temp_dir };

        let file_a_id = Uuid::new_v4();
        let file_b_id = Uuid::new_v4();

        let mut path_file_a = node_dir_two.clone();
        path_file_a.push(file_a_id.to_string());
        let file_a = path_file_a;

        let mut path_file_b = node_dir_two.clone();
        path_file_b.push(file_b_id.to_string());
        let file_b = path_file_b;

        let file_size = 1966924;
        debug!("Creating buffer of size {}", file_size);
        let mut random_bytes = vec![0u8; file_size as usize];

        debug!("Generating data");
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut random_bytes[..]);

        {
            debug!("Writing...");
            let mut file_a = File::create(file_a).await.expect("Test file crate failure");
            file_a
                .write_all(&random_bytes)
                .await
                .expect("Test file crate failure");
        }

        debug!("CA gen");
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
            port: 7671,
            own_id: Uuid::new_v4(),
        });

        let server_ledger = FragmentLedger::new(
            node_dir_one.to_str().unwrap().to_string(),
            16 * 1024 * 1024 * 1024,
            FileLockTable::new(5),
        );
        server_ledger
            .initialize()
            .await
            .expect("Ledger init failed");
        let server_handler: PacketHandlerRef = Arc::new(Mutex::new(Box::new(
            MeowithMDSFTPPacketHandler::new(server_ledger.clone(), u16::MAX as u32),
        )));

        let mut server = MDSFTPServer::new(
            connection_auth_context.clone(),
            conn_map.clone(),
            server_handler,
            Default::default(),
        )
        .await;
        assert!(server
            .start(&cert, &key, IpAddr::from_str("0.0.0.0").unwrap())
            .await
            .is_ok());

        let client_ledger = FragmentLedger::new(
            node_dir_two.to_str().unwrap().to_string(),
            16 * 1024 * 1024 * 1024,
            FileLockTable::new(5),
        );
        client_ledger
            .initialize()
            .await
            .expect("Ledger init failed");
        let client_handler: PacketHandlerRef = Arc::new(Mutex::new(Box::new(
            MeowithMDSFTPPacketHandler::new(client_ledger.clone(), u16::MAX as u32),
        )));
        let mut client_pool =
            MDSFTPPool::new(connection_auth_context, conn_map, Default::default());
        client_pool.set_packet_handler(client_handler).await;

        let uploaded_id: Uuid;

        {
            debug!("Testing upload");
            let start = Instant::now();
            let channel = client_pool.channel(&id1).await.unwrap();
            let reserve = channel
                .try_reserve(
                    file_size,
                    ReserveFlags {
                        auto_start: true,
                        durable: false,
                        temp: false,
                        overwrite: false,
                    },
                )
                .await
                .expect("Reserve failed");

            let handler = Box::new(MeowithMDSFTPChannelPacketHandler::new(
                client_ledger.clone(),
                16,
                u16::MAX as u32,
            ));
            let meta = client_ledger
                .fragment_meta(&file_a_id)
                .await
                .expect("Meta read fail");
            debug!("Fragment {:?}", meta);
            let reader = client_ledger
                .fragment_read_stream(&file_a_id)
                .await
                .expect("Read fail");
            let handle = channel
                .send_content(
                    reader,
                    meta.disk_content_size,
                    reserve.chunk_buffer,
                    handler,
                )
                .await
                .expect("Delegate failed");
            debug!("Awaiting handle...");
            handle.await;
            uploaded_id = reserve.chunk_id;
            let channel = client_pool.channel(&id1).await.unwrap();
            channel
                .commit(uploaded_id, CommitFlags::r#final())
                .await
                .unwrap();
            debug!("Took {:?}", start.elapsed());

            let recv_meta = server_ledger
                .fragment_meta(&reserve.chunk_id)
                .await
                .unwrap();
            assert_eq!(recv_meta.disk_content_size, meta.disk_content_size);
        }

        {
            debug!("Testing download");
            let start = Instant::now();
            let channel = client_pool.channel(&id1).await.unwrap();
            let file_ba = File::create(file_b.clone())
                .await
                .expect("Test file crate failure");

            let handler = Box::new(MeowithMDSFTPChannelPacketHandler::new(
                client_ledger.clone(),
                16,
                u16::MAX as u32,
            ));
            let handle = channel
                .retrieve_content(
                    Arc::new(Mutex::new(Box::pin(BufWriter::new(file_ba)))),
                    handler,
                    true,
                )
                .await
                .expect("Retrieve req reg failed");
            debug!("Handler setup");
            channel
                .retrieve_req(uploaded_id, 16)
                .await
                .expect("Retrieve req failed");

            debug!("Awaiting handle...");
            handle.await;
            debug!("Took {:?}", start.elapsed());

            let file_bb = File::open(file_b.clone())
                .await
                .expect("Test file crate failure");
            let meta = file_bb.metadata().await.expect("Test file crate failure");

            assert_eq!(meta.len(), file_size);
        }
    }
}

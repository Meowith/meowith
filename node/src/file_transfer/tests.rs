#[cfg(test)]
mod tests {
    use crate::file_transfer::channel_handler::MeowithMDSFTPChannelPacketHandler;
    use crate::file_transfer::packet_handler::MeowithMDSFTPPacketHandler;
    use crate::io::fragment_ledger::FragmentLedger;
    use crate::locking::file_lock_table::FileLockTable;
    use commons::autoconfigure::ssl_conf::{gen_test_ca, gen_test_certs};
    use commons::context::microservice_request_context::NodeAddrMap;
    use log::debug;
    use logging::initialize_test_logging;
    use ntest::timeout;
    use openssl::x509::X509VerifyResult;
    use protocol::file_transfer::authenticator::ConnectionAuthContext;
    use protocol::file_transfer::pool::{MDSFTPPool, PacketHandlerRef};
    use protocol::file_transfer::server::MDSFTPServer;
    use protocol::file_transfer::MAX_CHUNK_SIZE;
    use rand::RngCore;
    use std::collections::{HashMap, VecDeque};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::{env, fs, io};
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;
    use tokio::sync::{Mutex, RwLock};
    use uuid::Uuid;

    struct Cleanup {
        temp_dir: PathBuf,
    }

    fn remove_dir_iteratively(path: &PathBuf) -> io::Result<()> {
        let mut dirs_to_delete = VecDeque::new();
        let mut files_to_delete = Vec::new();

        dirs_to_delete.push_back(path.clone());

        while let Some(current_dir) = dirs_to_delete.pop_front() {
            for entry in fs::read_dir(&current_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    dirs_to_delete.push_back(path);
                } else {
                    files_to_delete.push(path);
                }
            }
        }

        for file in files_to_delete {
            fs::remove_file(file)?;
        }

        while let Some(dir) = dirs_to_delete.pop_back() {
            fs::remove_dir(dir)?;
        }

        Ok(())
    }

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = remove_dir_iteratively(&self.temp_dir).unwrap();
        }
    }

    #[tokio::test]
    #[timeout(2000)]
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
        let mut path_file_a = node_dir_two.clone();
        path_file_a.push(file_a_id.to_string());
        let file_a = path_file_a;
        let file_size = MAX_CHUNK_SIZE * 30 + 1024;
        let mut random_bytes = vec![0u8; file_size as usize];

        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut random_bytes[..]);

        {
            let mut file_a = File::create(file_a).await.expect("Test file crate failure");
            file_a
                .write_all(&random_bytes)
                .await
                .expect("Test file crate failure");
        }

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
            MeowithMDSFTPPacketHandler::new(server_ledger.clone()),
        )));

        let mut server = MDSFTPServer::new(
            connection_auth_context.clone(),
            conn_map.clone(),
            server_handler,
        )
        .await;
        assert!(server.start(&cert, &key).await.is_ok());

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
            MeowithMDSFTPPacketHandler::new(client_ledger.clone()),
        )));
        let mut client_pool = MDSFTPPool::new(connection_auth_context, conn_map);
        client_pool.set_packet_handler(client_handler).await;

        let _uploaded_id: Uuid;

        {
            let channel = client_pool.channel(&id1).await.unwrap();
            let reserve = channel
                .try_reserve(file_size, true)
                .await
                .expect("Reserve failed");

            let handler = Box::new(MeowithMDSFTPChannelPacketHandler::new(
                client_ledger.clone(),
                16,
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

            _uploaded_id = reserve.chunk_id;
            let recv_meta = server_ledger
                .fragment_meta(&reserve.chunk_id)
                .await
                .unwrap();
            assert_eq!(recv_meta.disk_content_size, meta.disk_content_size);
            assert_eq!(recv_meta.disk_physical_size, meta.disk_physical_size);
        }
    }
}

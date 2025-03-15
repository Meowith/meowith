use crate::io::fragment_metadata_store::{ExtFragmentMeta, ExtFragmentMetaStore};
use commons::error::io_error::{MeowithIoError, MeowithIoResult};
use sled::{Config, Db, Mode};
use std::path::Path;
use uuid::Uuid;

const SCHEMA_VERSION: u8 = 1;

pub struct EmbeddedFragmentMetaStore {
    db: Db,
    encoder_config: bincode::config::Configuration,
}

impl EmbeddedFragmentMetaStore {
    pub fn new(data_loc: &str) -> Self {
        let db_path = Path::new(data_loc).join("meta.db");
        let db = Config::new()
            .path(db_path)
            .mode(Mode::HighThroughput)
            .open()
            .expect("Failed to open meta.db");
        
        let schema = db.open_tree("schema").unwrap();
        if let Some(ver) = schema.get("version").unwrap() {
            let ver = ver[0];
            if ver != SCHEMA_VERSION {
                panic!(
                    "Metadata store schema version mismatch, expected version {} but got {}",
                    SCHEMA_VERSION, ver
                );
            }
        } else {
            schema.insert("version", &[SCHEMA_VERSION]).unwrap();
        }

        Self {
            db,
            encoder_config: bincode::config::Configuration::default(),
        }
    }
}

impl ExtFragmentMetaStore for EmbeddedFragmentMetaStore {
    fn insert(&self, chunk_id: Uuid, meta: &ExtFragmentMeta) -> MeowithIoResult<()> {
        let encoded: Vec<u8> = bincode::encode_to_vec(meta, self.encoder_config).unwrap();
        self.db.insert(chunk_id.as_bytes(), encoded)?;
        Ok(())
    }

    fn get(&self, chunk_id: Uuid) -> MeowithIoResult<ExtFragmentMeta> {
        let bytes = self
            .db
            .get(chunk_id.as_bytes())?
            .ok_or(MeowithIoError::NotFound)?;
        let obj: (ExtFragmentMeta, usize) =
            bincode::decode_from_slice(&bytes, self.encoder_config)?;
        Ok(obj.0)
    }

    fn remove(&self, chunk_id: Uuid) -> MeowithIoResult<()> {
        self.db.remove(chunk_id.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod embedded_fragment_metadata_store_tests {
    use super::*;
    use serial_test::serial;
    use uuid::Uuid;

    fn create_store() -> EmbeddedFragmentMetaStore {
        EmbeddedFragmentMetaStore::new("../tests/test_data/meta.db")
    }

    #[test]
    #[serial]
    fn test_reboot() {
        let store = create_store();
        let chunk_id = Uuid::new_v4();
        let meta = ExtFragmentMeta {
            bucket_id: 1234,
            file_id: 4567,
        };

        assert!(store.insert(chunk_id, &meta).is_ok());
        drop(store);
        let store = create_store();
        let retrieved = store.get(chunk_id).unwrap();
        assert_eq!(retrieved.bucket_id, meta.bucket_id);
        assert_eq!(retrieved.file_id, meta.file_id);
    }

    #[test]
    #[serial]
    fn test_insert_and_get() {
        let store = create_store();
        let chunk_id = Uuid::new_v4();
        let meta = ExtFragmentMeta {
            bucket_id: 123,
            file_id: 456,
        };

        assert!(store.insert(chunk_id, &meta).is_ok());
        let retrieved = store.get(chunk_id).unwrap();
        assert_eq!(retrieved.bucket_id, meta.bucket_id);
        assert_eq!(retrieved.file_id, meta.file_id);
    }

    #[test]
    #[serial]
    fn test_get_nonexistent() {
        let store = create_store();
        let chunk_id = Uuid::new_v4();
        assert!(store.get(chunk_id).is_err());
    }

    #[test]
    #[serial]
    fn test_remove() {
        let store = create_store();
        let chunk_id = Uuid::new_v4();
        let meta = ExtFragmentMeta {
            bucket_id: 123,
            file_id: 456,
        };

        assert!(store.insert(chunk_id, &meta).is_ok());
        assert!(store.remove(chunk_id).is_ok());
        assert!(store.get(chunk_id).is_err());
    }

    #[test]
    #[serial]
    fn test_remove_nonexistent() {
        let store = create_store();
        let chunk_id = Uuid::new_v4();
        assert!(store.remove(chunk_id).is_ok());
    }
}

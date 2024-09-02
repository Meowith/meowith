use data::pathlib::normalize;
use serde::Deserialize;
use uuid::Uuid;

pub mod entity_action;
pub mod entity_list;
pub mod file_transfer;

#[derive(Deserialize, Debug)]
pub struct EntryPath {
    pub app_id: Uuid,
    pub bucket_id: Uuid,
    path: Option<String>,
    #[serde(skip)]
    cached_path: Option<String>,
}

impl EntryPath {
    pub(crate) fn path(&mut self) -> String {
        if let Some(val) = &self.cached_path {
            val.clone()
        } else {
            self.cached_path = Some(normalize(&self.path.clone().unwrap_or_default()));
            self.cached_path.as_ref().unwrap().clone()
        }
    }
}

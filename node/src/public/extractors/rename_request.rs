use actix_web::dev::Payload;
use actix_web::{web, FromRequest, HttpRequest};
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::dto::config::FsLimitConfiguration;
use data::pathlib::prepare_path;
use serde::Deserialize;
use std::future::Future;
use std::pin::Pin;

#[derive(Deserialize, Debug)]
pub struct RenameEntityRequest {
    pub to: String,
    #[serde(skip)]
    cached_path: Option<String>,
}

impl RenameEntityRequest {
    pub fn check_valid(
        &mut self,
        fs_limit_configuration: &FsLimitConfiguration,
    ) -> NodeClientResponse<()> {
        let path = prepare_path(&self.to, fs_limit_configuration);
        if let Some(prepared_path) = path {
            self.cached_path = Some(prepared_path);
            Ok(())
        } else {
            Err(NodeClientError::BadRequest)
        }
    }

    pub fn path(&self) -> String {
        self.cached_path.as_ref().unwrap().clone()
    }
}

impl FromRequest for RenameEntityRequest {
    type Error = NodeClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let req_clone = req.clone();
        let fs_limit = req
            .app_data::<web::Data<FsLimitConfiguration>>()
            .cloned()
            .expect("FsLimitConfiguration not found");

        let entry_path_fut = web::Json::<RenameEntityRequest>::from_request(&req_clone, payload);

        Box::pin(async move {
            let mut entry_path: RenameEntityRequest = entry_path_fut
                .await
                .map_err(|_| NodeClientError::BadRequest)?
                .into_inner();

            entry_path
                .check_valid(&fs_limit)
                .map_err(|_| NodeClientError::BadResourcePath)?;

            Ok(entry_path)
        })
    }
}

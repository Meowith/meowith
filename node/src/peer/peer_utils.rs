use commons::context::microservice_request_context::MicroserviceRequestContext;
use commons::context::request_context::RequestContext;
use data::dto::controller::StorageResponse;

pub async fn fetch_peer_storage_info(
    req_ctx: &MicroserviceRequestContext,
) -> reqwest::Result<StorageResponse> {
    req_ctx
        .client()
        .await
        .get(req_ctx.controller("/api/internal/health/storage"))
        .send()
        .await?
        .json::<StorageResponse>()
        .await
}

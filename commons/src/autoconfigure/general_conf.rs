use crate::context::microservice_request_context::MicroserviceRequestContext;
use crate::context::request_context::RequestContext;
use data::dto::config::GeneralConfiguration;
use std::error::Error;

pub async fn fetch_general_config(
    ctx: &MicroserviceRequestContext,
) -> Result<GeneralConfiguration, Box<dyn Error>> {
    Ok(ctx
        .client()
        .await
        .get(ctx.controller("/api/internal/autoconfigure/config"))
        .send()
        .await?
        .json::<GeneralConfiguration>()
        .await?)
}

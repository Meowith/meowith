use std::future::{ready, Ready};

use actix_web::web::Data;
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;

use crate::error::node::NodeError;
use crate::AppState;

pub struct NodeVerify;

impl<S, B> Transform<S, ServiceRequest> for NodeVerify
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = NodeVerifyMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(NodeVerifyMiddleware { service }))
    }
}

pub struct NodeVerifyMiddleware<S> {
    service: S,
}

fn remove_bearer_prefix(token: &str) -> String {
    if let Some(stripped) = token.strip_prefix("Bearer ") {
        stripped.to_string()
    } else {
        token.to_string()
    }
}

impl<S, B> Service<ServiceRequest> for NodeVerifyMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let app_data = req.app_data::<Data<AppState>>().unwrap();
        let token_header = req.headers().get("Authorization");
        if token_header.is_none() {
            return Box::pin(async { Err(Error::from(NodeError::BadAuth)) });
        }
        let token_str = token_header.unwrap().to_str();
        if token_str.is_err() {
            return Box::pin(async { Err(Error::from(NodeError::BadAuth)) });
        }

        let clean_token = remove_bearer_prefix(token_str.unwrap());
        let node = app_data.req_ctx.token_node.get(&clean_token).cloned();
        if node.is_none() {
            return Box::pin(async { Err(Error::from(NodeError::BadAuth)) });
        }

        let user_obj = node.unwrap();
        req.extensions_mut().insert(user_obj);
        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}

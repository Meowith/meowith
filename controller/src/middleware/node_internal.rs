use crate::error::node::NodeError;
use crate::AppState;
use actix_web::web::Data;
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use data::model::microservice_node_model::MicroserviceNode;
use futures_util::future::LocalBoxFuture;
use std::future::{ready, Ready};
use std::rc::Rc;

pub struct NodeVerify;

impl<S: 'static, B> Transform<S, ServiceRequest> for NodeVerify
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
        ready(Ok(NodeVerifyMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct NodeVerifyMiddleware<S> {
    service: Rc<S>,
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
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.service.clone();

        Box::pin(async move {
            let app_data = req.app_data::<Data<AppState>>().unwrap();
            let token_header = req.headers().get("Authorization");
            if token_header.is_none() {
                return Err(Error::from(NodeError::BadAuth));
            }
            let token_str = token_header.unwrap().to_str();
            if token_str.is_err() {
                return Err(Error::from(NodeError::BadAuth));
            }

            let clean_token = remove_bearer_prefix(token_str.unwrap());
            let node: Option<MicroserviceNode>;
            {
                let tk_map = app_data.req_ctx.token_node.read().await;
                node = tk_map.get(&clean_token).cloned();
            }
            if node.is_none() {
                return Err(Error::from(NodeError::BadAuth));
            }

            let user_obj = node.unwrap();
            req.extensions_mut().insert(user_obj);
            let fut = svc.call(req);
            let res = fut.await?;
            Ok(res)
        })
    }
}

use std::future::{ready, Ready};
use std::rc::Rc;

use crate::public::response::NodeClientError;
use crate::AppState;
use actix_web::dev::Payload;
use actix_web::web::Data;
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, FromRequest, HttpMessage, HttpRequest,
};
use commons::access_token_service::Permit;
use commons::middleware_actions::remove_bearer_prefix;
use futures_util::future::{err, ok, LocalBoxFuture};
use uuid::Uuid;

pub struct UserAuthenticate;

impl<S: 'static, B> Transform<S, ServiceRequest> for UserAuthenticate
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = UserAuthenticateMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(UserAuthenticateMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct UserAuthenticateMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for UserAuthenticateMiddleware<S>
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
                return Err(Error::from(NodeClientError::BadAuth));
            }
            let token_str = token_header.unwrap().to_str();
            if token_str.is_err() {
                return Err(Error::from(NodeClientError::BadAuth));
            }

            let clean_token = remove_bearer_prefix(token_str.unwrap());

            let claim_data = app_data.jwt_service.verify_token(clean_token.as_str());

            if claim_data.is_err() {
                return Err(Error::from(NodeClientError::BadAuth));
            }

            let claim_data = claim_data.unwrap();

            // TODO "verify nonce"

            req.extensions_mut().insert(BucketAccessor {
                permits: claim_data.perms,
                app_id: claim_data.app_id,
            });
            let fut = svc.call(req);
            let res = fut.await?;
            Ok(res)
        })
    }
}

struct BucketAccessor {
    permits: Vec<Permit>,
    app_id: Uuid,
}

impl FromRequest for BucketAccessor {
    type Error = NodeClientError;
    type Future = futures::future::Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        return match req.extensions_mut().remove::<BucketAccessor>() {
            Some(bucket_accessor) => ok(bucket_accessor),
            None => err(NodeClientError::BadAuth),
        };
    }
}

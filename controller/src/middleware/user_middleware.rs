use crate::AppState;
use actix_web::http::header::AUTHORIZATION;
use actix_web::web::Data;
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use commons::error::std_response::NodeClientError;
use commons::middleware_actions::remove_bearer_prefix;
use data::access::user_access::get_user_from_id;
use futures_util::future::LocalBoxFuture;
use std::future::{ready, Ready};
use std::rc::Rc;

pub struct UserMiddlewareRequestTransform;

impl<S, B> Transform<S, ServiceRequest> for UserMiddlewareRequestTransform
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = UserMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(UserMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct UserMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for UserMiddleware<S>
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
            let token_header = req.headers().get(AUTHORIZATION);
            if token_header.is_none() {
                return Err(Error::from(NodeClientError::BadAuth));
            }
            let token_str = token_header.unwrap().to_str();
            if token_str.is_err() {
                return Err(Error::from(NodeClientError::BadAuth));
            }
            let claims = app_data
                .auth_jwt_service
                .verify_token(&remove_bearer_prefix(token_str.unwrap()));

            if claims.is_err() {
                return Err(Error::from(NodeClientError::BadAuth));
            }

            let user = get_user_from_id(claims.unwrap().id, &app_data.session).await;

            if user.is_err() {
                return Err(Error::from(NodeClientError::BadAuth));
            }

            let user = user.unwrap();

            req.extensions_mut().insert(user);

            let fut = svc.call(req);

            let res = fut.await?;
            Ok(res)
        })
    }
}

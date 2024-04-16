use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::task::{Context, Poll};

use actix_web::body::BoxBody;
use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header;
use actix_web::{Error, HttpResponse};
use futures::ready;
use log::info;
use pin_project_lite::pin_project;

pin_project! {
    #[project = BearerAuthFutureProjected]
    pub enum BearerAuthFuture<S: Service<ServiceRequest>> {
        Ok {
            #[pin]
            ok_fut: S::Future,
        },
        Err {
            #[pin]
            err_fut: Ready<Result<S::Response, S::Error>>,
        }
    }
}

impl<S> Future for BearerAuthFuture<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error>,
    <S as Service<ServiceRequest>>::Future: Future,
{
    type Output = <S::Future as Future>::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            BearerAuthFutureProjected::Ok { ok_fut } => {
                let res = ready!(ok_fut.poll(cx));
                Poll::Ready(res)
            }

            BearerAuthFutureProjected::Err { err_fut } => {
                let res = ready!(err_fut.poll(cx));
                Poll::Ready(res)
            }
        }
    }
}

pub struct BearerAuth {
    token: String,
}

impl BearerAuth {
    pub fn new(token: String) -> Self {
        BearerAuth { token }
    }
}

impl<S> Transform<S, ServiceRequest> for BearerAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error>,
    S::Future: 'static,
{
    type Error = Error;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;
    type InitError = ();
    type Response = ServiceResponse<BoxBody>;
    type Transform = BearerAuthMiddleware<S>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(BearerAuthMiddleware {
            service,
            token: self.token.clone(),
        }))
    }
}

pub struct BearerAuthMiddleware<S> {
    service: S,
    token: String,
}

impl<S> Service<ServiceRequest> for BearerAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error>,
    S::Future: 'static,
{
    type Error = Error;
    type Future = BearerAuthFuture<S>;
    type Response = ServiceResponse<BoxBody>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let headers = req.request().headers();

        let auth_ok = headers
            .get(header::AUTHORIZATION)
            .and_then(|content| content.to_str().ok())
            .filter(|s| bearer_matches_token(s, &self.token))
            .is_some();

        {
            use std::io::{stdout, Write};
            stdout().flush().unwrap();
        }

        match auth_ok {
            true => BearerAuthFuture::Ok {
                ok_fut: self.service.call(req),
            },
            false => {
                let (req, _) = req.into_parts();
                let resp = ServiceResponse::new(req, HttpResponse::Unauthorized().body(()));
                let err_fut = ready(Ok(resp));
                BearerAuthFuture::Err { err_fut }
            }
        }
    }
}

fn bearer_matches_token(bearer: &str, token: &str) -> bool {
    info!("trying to match:\n{bearer}\nBearer {token}");
    let expected_bearer_len = token.len() + "Bearer ".len();
    bearer.len() == expected_bearer_len && bearer.get(7..).filter(|t| t == &token).is_some()
}

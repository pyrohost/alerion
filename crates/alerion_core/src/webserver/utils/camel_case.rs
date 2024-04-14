use futures::ready;
use pin_project_lite::pin_project;
use actix_web::dev::{forward_ready, Service, Transform, ServiceRequest, ServiceResponse};
use actix_web::Error;
use std::future::{ready, Ready, Future};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    pub struct DefaultHeaderFuture<S: Service<ServiceRequest>> {
        #[pin]
        fut: S::Future,
    }
}

impl<S, B> Future for DefaultHeaderFuture<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
{
    type Output = <S::Future as Future>::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let mut res = ready!(this.fut.poll(cx))?;

        let head = res.response_mut().head_mut();
        head.set_camel_case_headers(true);

        Poll::Ready(Ok(res))
    }
}

pub struct CamelCaseHeaders;

impl<S, B> Transform<S, ServiceRequest> for CamelCaseHeaders
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = CamelCaseHeadersMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CamelCaseHeadersMiddleware { service }))
    }
}

pub struct CamelCaseHeadersMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for CamelCaseHeadersMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = DefaultHeaderFuture<S>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let fut = self.service.call(req);

        DefaultHeaderFuture { fut }
    }
}

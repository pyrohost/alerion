use std::time::Instant;

use poem::{Endpoint, IntoResponse, Middleware, Request, Response};
use tracing::{Instrument, Level};

/// Rewritten middleware for [`tracing`](https://crates.io/crates/tracing),
/// originally from poem's `Tracing` middleware.
#[derive(Default)]
pub struct Tracing;

impl<E: Endpoint> Middleware<E> for Tracing {
    type Output = TracingEndpoint<E>;

    fn transform(&self, ep: E) -> Self::Output {
        TracingEndpoint { inner: ep }
    }
}

/// Endpoint for [`Tracing`] middleware.
pub struct TracingEndpoint<E> {
    inner: E,
}

impl<E: Endpoint> Endpoint for TracingEndpoint<E> {
    type Output = Response;

    async fn call(&self, req: Request) -> poem::Result<Self::Output> {
        let endpoint = format!("{} {}", req.method(), req.original_uri());

        let span = tracing::span!(
            target: module_path!(),
            Level::DEBUG,
            "request",
            endpoint = %endpoint,
        );

        async move {
            let now = Instant::now();
            let res = self.inner.call(req).await;
            let duration = now.elapsed();

            match res {
                Ok(resp) => {
                    let resp = resp.into_response();
                    tracing::info!(
                        status = %resp.status(),
                        duration = ?duration,
                        "response"
                    );
                    Ok(resp)
                }
                Err(err) => {
                    tracing::info!(
                        status = %err.status(),
                        error = %err,
                        duration = ?duration,
                        "error"
                    );
                    Err(err)
                }
            }
        }
        .instrument(span)
        .await
    }
}

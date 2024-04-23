use poem::{Endpoint, Middleware, Request};
use reqwest::{Method, StatusCode};

pub struct BearerAuthMiddleware {
    token: String,
}

impl BearerAuthMiddleware {
    pub fn new(token: String) -> Self {
        Self { token }
    }
}

impl<E: Endpoint> Middleware<E> for BearerAuthMiddleware {
    type Output = BearerAuthMiddlewareImpl<E>;

    fn transform(&self, ep: E) -> Self::Output {
        BearerAuthMiddlewareImpl {
            ep,
            token: self.token.clone(),
        }
    }
}

/// The new endpoint type generated by the TokenMiddleware.
pub struct BearerAuthMiddlewareImpl<E> {
    ep: E,
    token: String,
}

/// Token data
impl<E: Endpoint> Endpoint for BearerAuthMiddlewareImpl<E> {
    type Output = E::Output;

    async fn call(&self, req: Request) -> poem::Result<Self::Output> {
        println!("{req:#?}");

        if req.method() == Method::OPTIONS {
            return self.ep.call(req).await;
        }

        if let Some(value) = req
            .headers()
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
        {
            let token = value.to_string();

            if token == format!("Bearer {}", self.token) {
                self.ep.call(req).await
            } else {
                Err(poem::Error::from_string(
                    "Token does not match",
                    StatusCode::UNAUTHORIZED,
                ))
            }
        } else {
            Err(poem::Error::from_string(
                "No token provided",
                StatusCode::UNAUTHORIZED,
            ))
        }
    }
}

use std::borrow::Cow;
use actix_web::web;
use futures::StreamExt;

pub async fn use_payload_as_str<F>(mut payload: web::Payload, closure: F)
where
    F: FnOnce(actix_web::Result<Cow<str>>),
{
    const MAX_SIZE: usize = 262_144; // max payload size is 256k

    let mut body = web::BytesMut::new();
    let mut result = Ok(());

    while let Some(chunk) = payload.next().await {
        let chunk = chunk.unwrap();
        // limit max size of in-memory payload
        if (body.len() + chunk.len()) > MAX_SIZE {
            result = Err(actix_web::error::ErrorBadRequest("overflow"));
            break;
        }
        body.extend_from_slice(&chunk);
    }

    closure(result.map(|_| String::from_utf8_lossy(&body[..])));
}

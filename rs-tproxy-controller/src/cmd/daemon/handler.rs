

use actix_http::{body::Body, http::StatusCode};
use actix_http::{Error, Request, Response, Method};

use bytes::BytesMut;
use futures_util::StreamExt as _;

use crate::cmd::raw_config::RawConfig;

async fn _handle(mut req: Request) -> Result<Response<Body>, Error> {
    if req.method() != Method::PUT {
        return Ok(Response::build(StatusCode::METHOD_NOT_ALLOWED)
            .body(Body::Empty));
    }

    let mut body = BytesMut::new();
    while let Some(item) = req.payload().next().await {
        body.extend_from_slice(&item?)
    }

    let _ : RawConfig = serde_json::from_slice(body.as_ref()).unwrap();

    Ok(Response::build(StatusCode::OK)
        .body(Body::Empty))
}




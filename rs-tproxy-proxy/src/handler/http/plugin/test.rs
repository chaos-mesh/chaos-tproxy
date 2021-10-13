use std::io;

use futures::stream::TryStreamExt;
use futures::AsyncReadExt;
use http::Response;
use hyper::Body;
use serde::Deserialize;

use super::plugins::BASIC_PLUGIN;
use super::Plugin;

#[derive(Debug, Deserialize)]
struct Content {
    #[serde(rename(deserialize = "type"))]
    typ: String,
    content: Vec<u8>,
}

#[tokio::test]
async fn test_plugin() -> anyhow::Result<()> {
    let body = "Hello World";
    let content_type = "plain/text";
    let plugin = Plugin::WASM(base64::decode(BASIC_PLUGIN)?.into());
    let resp = Response::builder()
        .status(200)
        .header("content-type", content_type)
        .body(Body::from(body))?;
    let new_resp = plugin.handle_response(resp).await?;
    let mut body_data = Vec::new();
    new_resp
        .into_body()
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
        .into_async_read()
        .read_to_end(&mut body_data)
        .await?;
    let content: Content = serde_json::from_slice(&body_data)?;
    assert_eq!(content.typ, content_type);
    assert_eq!(content.content, body.as_bytes());
    Ok(())
}

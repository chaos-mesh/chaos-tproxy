use std::io;
use std::sync::Arc;

use futures::stream::TryStreamExt;
use futures::AsyncReadExt;
use http::Response;
use hyper::Body;
use serde::Deserialize;
use wasmer_runtime::compile;

use super::Plugin;

///
/// ## wasm plugin in base64
///
/// ```rust
/// use log::info;
/// use rs_tproxy_plugin::register_response_handler;
///
/// register_response_handler!(|resp| {
///     let content_type = resp
///         .headers()
///         .get("content-type")
///         .ok_or(anyhow::anyhow!("content-type not found"))?
///         .to_str()?;
///     info!("get content-type: {}", content_type);
///     Ok(serde_json::to_vec(&serde_json::json!({
///         "type": content_type,
///         "content": *resp.body(),
///     }))?)
/// });
///
/// ```
///
const PLUGIN: &[u8] =
    include_bytes!("../../../../../target/wasm32-unknown-unknown/debug/plugin_example.wasm");

#[derive(Debug, Deserialize)]
struct Content {
    #[serde(rename(deserialize = "type"))]
    typ: String,
    content: Vec<u8>,
}

#[tokio::test]
async fn test_plugin() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::level_filters::LevelFilter::INFO)
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|err| anyhow::anyhow!("{}", err))?;
    let body = "Hello World";
    let content_type = "plain/text";
    let plugin = Plugin::WASM(Arc::new(compile(PLUGIN)?));
    let resp = Response::builder()
        .status(200)
        .header("content-type", content_type)
        .body(Body::from(body))?;
    let start = std::time::SystemTime::now();
    let new_resp = plugin.handle_response(resp).await?;
    log::info!("elapsed: {}ms", start.elapsed()?.as_millis());
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

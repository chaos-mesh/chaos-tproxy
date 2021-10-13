use log::info;
use rs_tproxy_plugin::register_response_handler;

fn response_handler(resp: http::Response<&[u8]>) -> anyhow::Result<Vec<u8>> {
    let content_type = resp
        .headers()
        .get("content-type")
        .ok_or(anyhow::anyhow!("content-type not found"))?
        .to_str()?;
    info!("get content-type: {}", content_type);
    Ok(serde_json::to_vec(&serde_json::json!({
        "type": content_type,
        "content": *resp.body(),
    }))?)
}

register_response_handler!(response_handler);

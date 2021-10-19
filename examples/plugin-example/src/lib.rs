use log::info;
use rs_tproxy_plugin::register_response_handler;

register_response_handler!(|resp| {
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
});

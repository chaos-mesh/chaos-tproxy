use log::info;
use rs_tproxy_plugin::logger::setup_logger;
use rs_tproxy_plugin::{read_response, write_body};

#[no_mangle]
pub extern "C" fn handle_response(ptr: i64, header_len: i64, body_len: i64) {
    setup_logger().unwrap();
    info!("success to setup logger");
    let resp = read_response(ptr, header_len, body_len).unwrap();
    let content_type = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    let new_body = serde_json::to_vec(&serde_json::json!({
        "type": content_type,
        "content": *resp.body(),
    }))
    .unwrap();
    write_body(new_body);
}

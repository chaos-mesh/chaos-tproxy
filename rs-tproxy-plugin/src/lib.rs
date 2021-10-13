use std::collections::HashMap;

use serde::Deserialize;

extern "C" {
    fn write_body(ptr: *const u8, len: usize);
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestHeader {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub header_map: HashMap<String, Vec<Vec<u8>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseHeader {
    pub status_code: u16,
    pub version: String,
    pub header_map: HashMap<String, Vec<Vec<u8>>>,
}

#[no_mangle]
pub extern "C" fn handle_response(ptr: i64, header_len: i64, body_len: i64) {
    unsafe {
        let header = std::slice::from_raw_parts(ptr as _, header_len as _);
        let body: &[u8] =
            std::slice::from_raw_parts((ptr + header_len) as _, (ptr + header_len + body_len) as _);
        let resp_header: ResponseHeader = serde_json::from_slice(header).unwrap();
        let content_type =
            std::str::from_utf8(&resp_header.header_map.get("content-type").unwrap()[0]).unwrap();
        let new_body = serde_json::to_vec(&serde_json::json!({
            "type": content_type,
            "content": body,
        }))
        .unwrap();
        write_body(new_body.as_ptr(), new_body.len());
    }
}

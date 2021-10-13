use http::{Request, Response};

mod buildin {
    extern "C" {
        pub fn write_body(ptr: *const u8, len: usize);
    }
}

pub mod header;
pub mod logger;
pub mod print;

pub fn read_request<'a>(
    ptr: i64,
    header_len: i64,
    body_len: i64,
) -> anyhow::Result<Request<&'a [u8]>> {
    let header = unsafe { std::slice::from_raw_parts(ptr as _, header_len as _) };
    let body: &[u8] = unsafe { std::slice::from_raw_parts((ptr + header_len) as _, body_len as _) };
    let req_header: header::RequestHeader = serde_json::from_slice(header)?;
    Ok(req_header.build(body)?)
}

pub fn read_response<'a>(
    ptr: i64,
    header_len: i64,
    body_len: i64,
) -> anyhow::Result<Response<&'a [u8]>> {
    let header = unsafe { std::slice::from_raw_parts(ptr as _, header_len as _) };
    let body: &[u8] = unsafe { std::slice::from_raw_parts((ptr + header_len) as _, body_len as _) };
    let resp_header: header::ResponseHeader = serde_json::from_slice(header)?;
    Ok(resp_header.build(body)?)
}

pub fn write_body(body: Vec<u8>) {
    unsafe { buildin::write_body(body.as_ptr(), body.len()) };
}

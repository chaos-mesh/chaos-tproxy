use http::{Request, Response};
use log::error;

use super::header::{RequestHeader, ResponseHeader};
use super::logger::setup_logger;
use super::print::eprintln;

#[macro_export]
macro_rules! register_request_handler {
    ($func_name:ident) => {
        #[no_mangle]
        pub extern "C" fn handle_request(ptr: i64, header_len: i64, body_len: i64) {
            $crate::call_request_handler(ptr, header_len, body_len, $func_name)
        }
    };
}

#[macro_export]
macro_rules! register_response_handler {
    ($func_name:ident) => {
        #[no_mangle]
        pub extern "C" fn handle_response(ptr: i64, header_len: i64, body_len: i64) {
            $crate::call_response_handler(ptr, header_len, body_len, $func_name)
        }
    };
}

pub fn call_response_handler<F>(ptr: i64, header_len: i64, body_len: i64, handler: F)
where
    F: Fn(Response<&[u8]>) -> anyhow::Result<Vec<u8>>,
{
    if let Err(err) = setup_logger() {
        eprintln(format!("plugin fail to setup logger: {}", err));
    }

    let scope = || -> anyhow::Result<()> {
        let resp = read_response(ptr, header_len, body_len)?;
        write_body(handler(resp)?);
        Ok(())
    };

    if let Err(err) = scope() {
        error!("fail to call response handler: {}", err)
    }
}

pub fn call_request_handler<F>(ptr: i64, header_len: i64, body_len: i64, handler: F)
where
    F: Fn(Request<&[u8]>) -> anyhow::Result<Vec<u8>>,
{
    if let Err(err) = setup_logger() {
        eprintln(format!("plugin fail to setup logger: {}", err));
    }

    let scope = || -> anyhow::Result<()> {
        let req = read_request(ptr, header_len, body_len)?;
        write_body(handler(req)?);
        Ok(())
    };

    if let Err(err) = scope() {
        error!("fail to call response handler: {}", err)
    }
}

pub fn read_request<'a>(
    ptr: i64,
    header_len: i64,
    body_len: i64,
) -> anyhow::Result<Request<&'a [u8]>> {
    let header = unsafe { std::slice::from_raw_parts(ptr as _, header_len as _) };
    let body: &[u8] = unsafe { std::slice::from_raw_parts((ptr + header_len) as _, body_len as _) };
    let req_header: RequestHeader = serde_json::from_slice(header)?;
    Ok(req_header.build(body)?)
}

pub fn read_response<'a>(
    ptr: i64,
    header_len: i64,
    body_len: i64,
) -> anyhow::Result<Response<&'a [u8]>> {
    let header = unsafe { std::slice::from_raw_parts(ptr as _, header_len as _) };
    let body: &[u8] = unsafe { std::slice::from_raw_parts((ptr + header_len) as _, body_len as _) };
    let resp_header: ResponseHeader = serde_json::from_slice(header)?;
    Ok(resp_header.build(body)?)
}

mod buildin {
    extern "C" {
        pub fn write_body(ptr: *const u8, len: usize);
    }
}

pub fn write_body(body: Vec<u8>) {
    unsafe { buildin::write_body(body.as_ptr(), body.len()) };
}

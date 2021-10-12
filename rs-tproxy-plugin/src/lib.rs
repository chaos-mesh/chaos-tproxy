extern "C" {
    fn write_body(ptr: *const u8, len: usize);
}

#[no_mangle]
pub extern "C" fn handle_response(ptr: i64, header_len: i64, body_len: i64) {
    unsafe {
        let header = std::slice::from_raw_parts(ptr as _, header_len as _);
        let body =
            std::slice::from_raw_parts((ptr + header_len) as _, (ptr + header_len + body_len) as _);
        let header_str = std::str::from_utf8_unchecked(header);
        let body_str = std::str::from_utf8_unchecked(body);
        let new_body_str = format!("header: {}\nbody: {}", header_str, body_str);
        write_body(new_body_str.as_ptr(), new_body_str.len());
    }
}

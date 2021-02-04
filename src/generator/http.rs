use nom::AsBytes;
use httparse::Header;

pub fn generate_http_request<'a>(req : &'a httparse::Request, length: usize) -> Vec<u8> {
    let mut buf:Vec<u8> = generate_http_request_first_line(req);
    buf.extend(b"\r\n");
    buf = generate_http_header(req.headers.as_ref(),buf);
    buf.extend(b"\r\n");
    buf
}

pub fn generate_http_request_first_line<'a>(req : &'a httparse::Request) -> Vec<u8> {
    [
        req.method.unwrap().as_bytes(),
        b" ",
        req.path.unwrap().as_bytes(),
        b" HTTP/1.",
        req.version.unwrap().to_string().as_bytes(),
    ].concat()
}

pub fn generate_http_response<'a>(rsp : &'a httparse::Response, length: usize) -> Vec<u8> {
    let mut buf:Vec<u8> = generate_http_response_first_line(rsp);
    buf.extend(b"\r\n");
    buf = generate_http_header(rsp.headers.as_ref(),buf);
    buf.extend(b"\r\n");
    buf
}

pub fn generate_http_response_first_line<'a>(rsp: &'a httparse::Response) -> Vec<u8> {
    [
        b"HTTP/1.",
        rsp.version.unwrap().to_string().as_bytes(),
        b" ",
        rsp.code.unwrap().to_string().as_bytes(),
        b" ",
        rsp.reason.unwrap().as_bytes(),
    ].concat()
}

pub fn generate_http_header(headers:&[Header], mut buf:Vec<u8>) -> Vec<u8> {
    for i in 0..headers.len() {
        buf.extend(headers[i].name.as_bytes());
        buf.extend(b": ");
        buf.extend(headers[i].value.as_bytes());
        buf.extend(b"\r\n");
    }
    buf
}

#[test]
fn test() {
    let req = httparse::Request{
        method: Some("GET"),
        path: Some("/404"),
        version: Some(1),
        headers: &mut [Header{ name: "Host", value: b"a" }]
    };
    println!("{:?}",String::from_utf8(generate_http_request(&req,0)));
    let rsp = httparse::Response{
        version: Some(1),
        code: Some(200),
        reason: Some("OK"),
        headers: &mut [Header{ name: "Host", value: b"a" }]
    };
    println!("{:?}",String::from_utf8(generate_http_response(&rsp,0)));
}
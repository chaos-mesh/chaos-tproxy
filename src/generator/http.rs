use nom::AsBytes;
use crate::parser::http::header::{RequestLine, StatusLine, HeaderField, StartLine, Version};
use crate::parser::http::message::{HttpMessage, HttpState};
use std::str::from_utf8;
use crate::parser::http::body::BodyState;

pub fn gen_http_header(message: &HttpMessage) -> Vec<u8> {
    let mut buf: Vec<u8> = match &message.start_line {
        StartLine::Request(request_line) => {
            gen_request_line(&request_line)
        }
        StartLine::Status(status_line) => {
            gen_status_line(&status_line)
        }
    };
    buf = gen_header_fields(&message.header_fields, buf);
    buf.extend(b"\r\n");
    buf
}

pub fn gen_request_line(request_line: &RequestLine) -> Vec<u8> {
    [
        request_line.method,
        b" ",
        request_line.path,
        b" HTTP/",
        request_line.version.major.as_slice(),
        b".",
        request_line.version.minor.as_slice(),
        b"\r\n",
    ].concat()
}

pub fn gen_status_line(status_line: &StatusLine) -> Vec<u8> {
    match status_line.reason_phrase {
        None => {
            [
                b"HTTP/",
                status_line.version.major.as_slice(),
                b".",
                status_line.version.minor.as_slice(),
                b" ",
                status_line.code,
                b"\r\n",
            ].concat()
        }
        Some(_) => {
            [
                b"HTTP/",
                status_line.version.major.as_slice(),
                b".",
                status_line.version.minor.as_slice(),
                b" ",
                status_line.code,
                b" ",
                status_line.reason_phrase.unwrap(),
                b"\r\n",
            ].concat()
        }
    }
}

pub fn gen_header_fields(header_fields: &Vec<HeaderField>, mut buf: Vec<u8>) -> Vec<u8> {
    for i in 0..header_fields.len() {
        buf.extend(header_fields[i].field_name);
        buf.extend(b": ");
        buf.extend(header_fields[i].field_value);
        buf.extend(b"\r\n");
    }
    buf
}

#[test]
fn test() {
    let message = HttpMessage {
        start_line: StartLine::Request(RequestLine {
            method: &b"GET"[..],
            path: &b"/psrse"[..],
            version: Version { major: b"1".to_vec(), minor: b"1".to_vec() },
        }),
        header_fields: vec![HeaderField { field_name: &b"a"[..], field_value: &b"c"[..] }],
        body_state: BodyState::Complete,
    };
    assert_eq!(gen_http_header(&message).as_slice(), b"GET /psrse HTTP/1.1\r\na: c\r\n\r\n");
}
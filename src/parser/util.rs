use nom::bytes::streaming::{take_while,tag};
use nom::{Err, error::{Error, ErrorKind}, IResult};

#[inline]
fn is_newline(chr: u8) -> bool {
    chr == b'\n' || chr == b'\r'
}

pub fn take_http(b: &[u8]) -> IResult<&[u8], &[u8]> {
    let (result, _) = take_while(is_newline)(b)?;
    tag("HTTP")(result)
}

#[test]
fn test_take_http() {
    let buf = b"\r\n\n\r\nHTTP";
    assert_eq!(take_http(buf),Ok((&b""[..],&b"HTTP"[..])));
    let buf = b"\r\nGET";
    assert_eq!(take_http(buf),Err(Err::Error(Error::new(&b"GET"[..], ErrorKind::Tag))));
    let buf = b"GET";
    assert_eq!(take_http(buf),Err(Err::Error(Error::new(&b"GET"[..], ErrorKind::Tag))));
}
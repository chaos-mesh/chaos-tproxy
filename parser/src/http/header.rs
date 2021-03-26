use std::fmt::Debug;

use httparse::{is_header_name_token, is_header_value_token, is_token, is_uri_token};
use nom::bytes::complete::take_while_m_n;
use nom::bytes::streaming::{tag, take_while, take_while1};
use nom::character::complete::digit0;
use nom::character::is_digit;
use nom::character::streaming::space0;
use nom::combinator::{map, opt};
use nom::multi::many_till;
use nom::sequence::{pair, tuple};
use nom::IResult;

pub fn is_cr_or_lf(chr: u8) -> bool {
    chr == b'\r' || chr == b'\n'
}

pub fn skip_cr_or_lf(i: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while(is_cr_or_lf)(i)
}

pub fn in_space<F, I, O>(f: F) -> impl FnMut(I) -> IResult<I, O>
where
    I: nom::InputTakeAtPosition + Clone,
    <I as nom::InputTakeAtPosition>::Item: nom::AsChar + Clone,
    F: Fn(I) -> IResult<I, O>,
{
    map(tuple((space0, f, space0)), |(_, x, _)| x)
}

#[derive(Debug, Eq, PartialEq)]
pub struct Version {
    pub major: Vec<u8>,
    pub minor: Vec<u8>,
}

pub fn version(i: &[u8]) -> IResult<&[u8], Version> {
    let (i, _) = tag(b"HTTP/")(i)?;
    let (i, (major, _, minor)) = tuple((digit0, tag(b"."), digit0))(i)?;
    Ok((
        i,
        Version {
            major: major.to_vec(),
            minor: minor.to_vec(),
        },
    ))
}

#[derive(Debug, Eq, PartialEq)]
pub struct RequestLine<'a> {
    /// The request method, such as `GET`.
    pub method: &'a [u8],
    /// The request path, such as `/about-us`.
    pub path: &'a [u8],
    /// The request version, such as `HTTP/1.1`.
    pub version: Version,
}

pub fn method(i: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while1(is_token)(i)
}

pub fn path(i: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while1(is_uri_token)(i)
}

pub fn request_line(i: &[u8]) -> IResult<&[u8], StartLine> {
    map(
        tuple((method, tag(b" "), path, tag(b" "), version, tag(b"\r\n"))),
        |(method, _, path, _, version, _)| {
            StartLine::Request(RequestLine {
                method,
                path,
                version,
            })
        },
    )(i)
}

pub fn status_code(i: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while_m_n(3, 3, is_digit)(i)
}

pub fn is_valid_reason_token(chr: u8) -> bool {
    chr == 0x09 || chr == b' ' || (chr >= 0x21 && chr <= 0x7E) || chr >= 0x80
}

pub fn reason_phrase(i: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while(is_valid_reason_token)(i)
}

#[derive(Debug, Eq, PartialEq)]
pub struct StatusLine<'a> {
    /// The response version, such as `HTTP/1.1`.
    pub version: Version,
    /// The response status-code, such as `500`, status-code = 3DIGIT.
    pub code: &'a [u8],
    /// The response reason phrase, reason-phrase = *( HTAB / SP / VCHAR / obs-text ), such as `Not Found`.
    pub reason_phrase: Option<&'a [u8]>,
}

pub fn status_line(i: &[u8]) -> IResult<&[u8], StartLine> {
    let (i, version) = version(i)?;
    let (i, _) = tag(b" ")(i)?;
    let (i, code) = status_code(i)?;
    let (i, reason_phrase) = map(
        pair(
            opt(map(pair(tag(b" "), reason_phrase), |it| it.1)),
            tag(b"\r\n"),
        ),
        |it| it.0,
    )(i)?;
    Ok((
        i,
        StartLine::Status(StatusLine {
            version,
            code,
            reason_phrase,
        }),
    ))
}

#[derive(Debug, Eq, PartialEq)]
pub struct HeaderField<'a> {
    pub field_name: &'a [u8],
    pub field_value: &'a [u8],
}

pub fn header_field(i: &[u8]) -> IResult<&[u8], HeaderField> {
    map(
        tuple((
            take_while(is_header_name_token),
            tag(b":"),
            in_space(take_while(is_header_value_token)),
            tag(b"\r\n"),
        )),
        |(name, _, value, _)| HeaderField {
            field_name: name,
            field_value: value,
        },
    )(i)
}

pub fn header_fields(i: &[u8]) -> IResult<&[u8], Vec<HeaderField>> {
    let (i, (header_fields, _)) = many_till(header_field, tag(b"\r\n"))(i)?;
    Ok((i, header_fields))
}

#[derive(Debug, Eq, PartialEq)]
pub enum StartLine<'a> {
    Request(RequestLine<'a>),
    Status(StatusLine<'a>),
}

#[cfg(test)]
mod tests {
    use crate::http::header::*;

    #[test]
    fn test_skip_cr_or_lf() {
        assert_eq!(skip_cr_or_lf(b"GET "), Ok((&b"GET "[..], &b""[..])));
        assert_eq!(
            skip_cr_or_lf(b"\r\n\r\n\n\rGET "),
            Ok((&b"GET "[..], &b"\r\n\r\n\n\r"[..]))
        );
    }

    #[test]
    fn test_method() {
        assert_eq!(method(b"GET "), Ok((&b" "[..], &b"GET"[..])));
    }

    #[test]
    fn test_path() {
        assert_eq!(path(b"/parser "), Ok((&b" "[..], &b"/parser"[..])));
    }

    #[test]
    fn test_version() {
        assert_eq!(
            version(b"HTTP/1.1 "),
            Ok((
                &b" "[..],
                Version {
                    major: b"1".to_vec(),
                    minor: b"1".to_vec()
                }
            ))
        );
    }

    #[test]
    fn test_header_field() {
        assert_eq!(
            header_field(b"a:b\r\n"),
            Ok((
                &b""[..],
                HeaderField {
                    field_name: &b"a"[..],
                    field_value: &b"b"[..]
                }
            ))
        )
    }

    #[test]
    fn test_header_fields() {
        assert_eq!(
            header_fields(b"a:b\r\nac:bc\r\n\r\n"),
            Ok((
                &b""[..],
                vec!(
                    HeaderField {
                        field_name: &b"a"[..],
                        field_value: &b"b"[..]
                    },
                    HeaderField {
                        field_name: &b"ac"[..],
                        field_value: &b"bc"[..]
                    }
                )
            ))
        );
        assert_eq!(header_fields(b"\r\n"), Ok((&b""[..], vec!())));
    }

    #[test]
    fn test_request_line() {
        let rl = b"Get /parser HTTP/1.1\r\n";
        assert_eq!(
            request_line(&rl[..]),
            Ok((
                &b""[..],
                StartLine::Request(RequestLine {
                    method: &rl[..3],
                    path: &rl[4..11],
                    version: Version {
                        major: b"1".to_vec(),
                        minor: b"1".to_vec()
                    },
                })
            ))
        );
    }

    #[test]
    fn test_status_line() {
        let rl = b"HTTP/1.1 200 OK!\r\n";
        assert_eq!(
            status_line(&rl[..]),
            Ok((
                &b""[..],
                StartLine::Status(StatusLine {
                    version: Version {
                        major: b"1".to_vec(),
                        minor: b"1".to_vec()
                    },
                    code: &rl[9..12],
                    reason_phrase: Some(&rl[13..16]),
                })
            ))
        )
    }
}

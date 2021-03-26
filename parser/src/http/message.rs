use nom::branch::alt;
use nom::error::Error;
use nom::IResult;

use crate::http::body::{body, body_state, BodyState};
use crate::http::header::{header_fields, request_line, status_line, HeaderField, StartLine};

#[derive(Debug, Eq, PartialEq)]
pub struct HttpMessage<'a> {
    pub start_line: StartLine<'a>,
    pub header_fields: Vec<HeaderField<'a>>,
    pub body_state: BodyState,
}

#[derive(Debug, Eq, PartialEq)]
pub enum HttpState<'a> {
    Incomplete(HttpMessage<'a>),
    Complete(HttpMessage<'a>),
}

// return body + state
pub fn http_state(i: &[u8]) -> IResult<(&[u8], &[u8], &[u8]), HttpState, Error<&[u8]>> {
    let (res, start_line) = alt((request_line, status_line))(i)?;
    println!("start_line OK");
    let (res, header_fields) = match header_fields(res) {
        Ok((res, header_fields)) => (res, header_fields),
        Err(e) => {
            println!("{:?}", e);
            return Err(e);
        }
    };
    println!("header_fields OK");
    let body_state = body_state(&header_fields);
    match body(res, body_state) {
        Ok((other_packet_res, BodyState::Complete)) => Ok((
            (
                &i[..i.len() - res.len()],
                &res[..res.len() - other_packet_res.len()],
                other_packet_res,
            ),
            HttpState::Complete(HttpMessage {
                start_line,
                header_fields,
                body_state: BodyState::Complete,
            }),
        )),
        Ok((other_packet_res, state)) => Ok((
            (
                &i[..i.len() - res.len()],
                &res[..res.len() - other_packet_res.len()],
                other_packet_res,
            ),
            HttpState::Incomplete(HttpMessage {
                start_line,
                header_fields,
                body_state: state,
            }),
        )),
        Err(e) => Err(e),
    }
}

#[test]
fn test_http_message() {
    let rl = b"Get /parser HTTP/1.1\r\nTransfer-Encoding:chunked\r\n\r\n4\r\n1111\r\n0\r\n\r\na";
    assert_eq!(
        http_state(&rl[..]),
        Ok((
            (
                &b"Get /parser HTTP/1.1\r\nTransfer-Encoding:chunked\r\n\r\n"[..],
                &b"4\r\n1111\r\n0\r\n\r\n"[..],
                &b"a"[..]
            ),
            HttpState::Complete(HttpMessage {
                start_line: StartLine::Request(RequestLine {
                    method: &rl[..3],
                    path: &rl[4..11],
                    version: Version {
                        major: b"1".to_vec(),
                        minor: b"1".to_vec()
                    },
                }),
                header_fields: vec!(HeaderField {
                    field_name: b"Transfer-Encoding",
                    field_value: b"chunked"
                }),
                body_state: BodyState::Complete
            })
        ))
    );
}

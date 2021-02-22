use crate::parser::http::header::{StartLine, HeaderField, request_line, status_line, header_fields, Version, RequestLine};
use nom::{Err::{self, Incomplete}, error::{Error, ErrorKind}, Needed, IResult};
use nom::branch::alt;
use crate::parser::http::body::{BodyState, body_state, body};


#[derive(Debug, Eq, PartialEq)]
pub struct HttpMessage<'a> {
    pub start_line: StartLine<'a>,
    pub header_fields: Vec<HeaderField<'a>>,
    pub state: HttpState,
}

#[derive(Debug, Eq, PartialEq)]
pub enum HttpState {
    BodyIncomplete(BodyState),
    Complete
}

pub fn http_message(i: &[u8]) -> IResult<&[u8], HttpMessage> {
    let (i, start_line) = alt((request_line, status_line))(i)?;
    let (i, header_fields) = header_fields(i)?;
    let body_state = body_state(&header_fields);
    match body(i,body_state) {
        Ok((i,BodyState::Complete)) => {
            Ok((i,
                HttpMessage {
                    start_line,
                    header_fields,
                    state: HttpState::Complete,
                }
            ))
        }
        Ok((i,state)) => {
            Ok((i,
                HttpMessage {
                    start_line,
                    header_fields,
                    state: HttpState::BodyIncomplete(state),
                }
            ))
        }
        Err(e) => {Err(e)}
    }
}

#[test]
fn test_http_message() {
    let rl = b"Get /parser HTTP/1.1\r\nTransfer-Encoding:chunked\r\n\r\n4\r\n1111\r\n0\r\n\r\n";
    assert_eq!(http_message(&rl[..]),
               Ok((
                   &b""[..], HttpMessage {
                       start_line: StartLine::Request(RequestLine {
                           method: &rl[..3],
                           path: &rl[4..11],
                           version: Version { major: b"1".to_vec(), minor: b"1".to_vec() },
                       }),
                       header_fields: vec!(
                           HeaderField { field_name: b"Transfer-Encoding", field_value: b"chunked"}
                       ),
                       state: HttpState::Complete
                   }
               ))
    );
}
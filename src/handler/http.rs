use crate::parser::http::message::{HttpMessage, HttpState, http_state};
use nom::IResult;
use crate::parser::http::header::{RequestLine, HeaderField, StartLine, StatusLine, header_fields};
use tokio::time::{sleep, Duration};
use std::sync::mpsc::{Sender, Receiver};
use crate::parser::http::body::{BodyState, body};
use nom::error::Error;
use crate::generator::http::gen_http_header;
use futures::{AsyncReadExt, AsyncWriteExt};
use Vec;
use std::collections::VecDeque;

pub enum PacketTarget {
    Request,
    Response,
}

pub struct Selector<'a> {
    path: &'a [u8],
    method: &'a [u8],
    code: &'a [u8],
    header_fields: Vec<HeaderField<'a>>,
}

pub fn select_request(
    request_line: &RequestLine,
    header_fields: &Vec<HeaderField>,
    selector: &Selector) -> bool {
    if request_line.path.starts_with(&selector.path) ||
        request_line.method.eq(selector.method) ||
        header_fields.iter().any(
            |x| selector.header_fields.iter().any(
                |y| y.field_name == x.field_name &&
                    y.field_value == x.field_value)
        ) {
        return true;
    }
    false
}

pub fn select_response(
    path: &[u8],
    method: &[u8],
    code: &[u8],
    header_fields: &Vec<HeaderField>,
    selector: &Selector) -> bool {
    if path.starts_with(&selector.path) ||
        method.eq(selector.method) ||
        code.eq(selector.code) ||
        header_fields.iter().any(
            |x| selector.header_fields.iter().any(
                |y| y.field_name == x.field_name &&
                    y.field_value == x.field_value)
        ) {
        return true;
    }
    false
}

pub enum Action<'a> {
    Replace(HttpMessage<'a>),
    Delay(Duration),
    Abort,
}

pub struct RequestInfo {
    path: Vec<u8>,
    method: Vec<u8>,
}

pub struct Handler<'a> {
    packet: PacketTarget,
    selector: Selector<'a>,
    action: Action<'a>,
    sender: Sender<RequestInfo>,
    receiver: Receiver<RequestInfo>,
}

impl Handler {
    pub fn handle_http_message(self, message: &HttpMessage) -> Option<Action> {
        return match &message.start_line {
            StartLine::Request(request_line) => {
                match self.packet {
                    PacketTarget::Request => {
                        if select_request(request_line, &message.header_fields, &self.selector) {
                            Some(self.action)
                        } else {
                            None
                        }
                    }
                    PacketTarget::Response => {
                        self.sender.send(RequestInfo {
                            path: request_line.path.to_vec(),
                            method: request_line.method.to_vec(),
                        });
                        None
                    }
                }
            }
            StartLine::Status(status_line) => {
                match self.packet {
                    PacketTarget::Request => {
                        None
                    }
                    PacketTarget::Response => {
                        match self.receiver.try_recv() {
                            Ok(request_info) => {
                                if select_response(
                                    request_info.path.as_slice(),
                                    request_info.method.as_slice(),
                                    status_line.code,
                                    &message.header_fields,
                                    &self.selector,
                                ) {
                                    Some(self.action)
                                } else {
                                    None
                                }
                            }
                            Err(e) => { None }
                        }
                    }
                }
            }
        };
    }

    pub fn handle_http(
        self,
        mut i: &[u8],
        mut body_state: BodyState,
    ) -> (IResult<(&[u8], &[u8], &[u8]), (BodyState, Option<Action>, Option<HttpMessage>)>) {
        return match body_state {
            BodyState::Complete => {
                match http_state(i) {
                    Ok(((header, body, rest), HttpState::Complete(http_message))) => {
                        Ok(((header, body, rest),
                            (BodyState::Complete, self.handle_http_message(&http_message), Some(http_message))))
                    }
                    Ok(((header, body, rest), HttpState::Incomplete(http_message))) => {
                        Ok(((header, body, rest),
                            (BodyState::Complete, self.handle_http_message(&http_message), Some(http_message))))
                    }
                    Err(e) => {
                        Err(e)
                    }
                }
            }
            _ => {
                match body(i, body_state) {
                    Ok((o, BodyState::Complete)) => {
                        Ok(((&i[..0], i, o), (state, None, None)))
                    }
                    Ok((o, state)) => {
                        Ok(((&i[..0], i, o), (state, None, None)))
                    }
                    Err(e) => { Err(e) }
                }
            }
        };
    }

    pub fn handle_stream<INPUT: AsyncReadExt, OUTPUT: AsyncWriteExt>
    (self, mut reader: INPUT, mut writer: OUTPUT) {
        let mut body_state = BodyState::Complete;
        loop {
            let mut buf_in = [u8; 4 * 1024usize];
            let n = match reader.read(&buf_in).await {
                // socket closed
                Ok(n) if n == 0 => return,
                Ok(n) => n,
                Err(e) => {
                    eprintln!("failed to read from socket; err = {:?}", e);
                    return;
                }
            };
            loop {
                match self.handle_http(&buf_in, body_state) {
                    Ok(((header, body, rest),
                           (state, action, http_message))) => {
                        match action {
                            Some(action) => {
                                match action {
                                    Action::Abort => {
                                        if let Err(e) = writer.write_all(header).await {
                                            eprintln!("failed to write to socket; err = {:?}", e);
                                            return;
                                        }
                                        if let Err(e) = writer.write_all(body).await {
                                            eprintln!("failed to write to socket; err = {:?}", e);
                                            return;
                                        }
                                        return;
                                    }
                                    Action::Delay(duration) => {
                                        sleep(duration);
                                        if let Err(e) = writer.write_all(header).await {
                                            eprintln!("failed to write to socket; err = {:?}", e);
                                            return;
                                        }
                                        if let Err(e) = writer.write_all(body).await {
                                            eprintln!("failed to write to socket; err = {:?}", e);
                                            return;
                                        }
                                    }
                                    Action::Replace(http_message) => {
                                        let message = gen_http_header(&http_message).as_slice();
                                        if let Err(e) = writer.write_all(message).await {
                                            eprintln!("failed to write to socket; err = {:?}", e);
                                            return;
                                        }
                                        if let Err(e) = writer.write_all(body).await {
                                            eprintln!("failed to write to socket; err = {:?}", e);
                                            return;
                                        }
                                    }
                                }
                            }
                            None => {
                                if let Err(e) = writer.write_all(header).await {
                                    eprintln!("failed to write to socket; err = {:?}", e);
                                    return;
                                }
                                if let Err(e) = writer.write_all(body).await {
                                    eprintln!("failed to write to socket; err = {:?}", e);
                                    return;
                                }
                            }
                        };
                        body_state = state;
                        if rest.is_empty() {
                            break;
                        }
                    }
                    Err(e) => {
                        println!(e);
                        if let Err(e) = writer.write_all(&buf_in).await {
                            eprintln!("failed to write to socket; err = {:?}", e);
                            return;
                        }
                        body_state = BodyState::Complete;
                        break;
                    }
                };
            }
        }
    }
}



#[test]
fn testtt() {
    let header_fields = vec![1, 2, 3];
    let header_fields0 = vec![2, 4, 5];
    assert_eq!(header_fields.iter().any(
        |x| header_fields0.iter().any(|y| y == x)), true);
}
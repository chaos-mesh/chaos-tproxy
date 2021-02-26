use crate::generator::http::gen_http_header;
use crate::parser::http::body::{body, BodyState};
use crate::parser::http::header::{header_fields, HeaderField, RequestLine, StartLine, StatusLine};
use crate::parser::http::message::{http_state, HttpMessage, HttpState};
use tokio::io::{AsyncWriteExt,AsyncReadExt};
use crossbeam::channel::{Sender,Receiver};
use serde_derive::Deserialize;

use nom::{
    error::{Error, ErrorKind},
    Err::{self, Incomplete},
    IResult, Needed,
};
use std::io;
use tokio::time::{sleep, Duration};
use std::future::{Future, Pending, Ready};


#[derive(Debug, Eq, PartialEq, Clone, Copy, Deserialize)]
pub enum PacketTarget {
    Request,
    Response,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct HeaderFieldVec {
    pub field_name: Vec<u8>,
    pub field_value: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct Selector {
    pub path: Option<Vec<u8>>,
    pub method: Option<Vec<u8>>,
    pub code: Option<Vec<u8>>,
    pub header_fields: Option<Vec<HeaderFieldVec>>,
}

pub fn select_request(
    request_line: &RequestLine,
    header_fields: &Vec<HeaderField>,
    selector: &Selector,
) -> bool {
    if match &selector.path {
        Some(p) => request_line.path.starts_with(p.as_slice()),
        None => false,
    } || match &selector.method {
        Some(m) => request_line.method.eq(m.as_slice()),
        None => false,
    } || match &selector.header_fields {
        Some(fields) => header_fields.iter().any(|x| {
            fields
                .iter()
                .any(|y| y.field_name == x.field_name && y.field_value == x.field_value)
        }),
        None => false,
    } {
        return true;
    }
    false
}

pub fn select_response(
    path: &[u8],
    method: &[u8],
    code: &[u8],
    header_fields: &Vec<HeaderField>,
    selector: &Selector,
) -> bool {
    if match &selector.path {
        Some(p) => path.starts_with(p.as_slice()),
        None => false,
    } || match &selector.method {
        Some(m) => method.eq(m.as_slice()),
        None => false,
    } || match &selector.code {
        Some(c) => code.eq(c.as_slice()),
        None => false,
    } || match &selector.header_fields {
        Some(fields) => header_fields.iter().any(|x| {
            fields
                .iter()
                .any(|y| y.field_name == x.field_name && y.field_value == x.field_value)
        }),
        None => false,
    } {
        return true;
    }
    false
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub enum Action {
    Replace(Vec<u8>),
    Delay(Duration),
    Abort,
}

#[derive(Debug, Eq, PartialEq)]
pub struct RequestInfo {
    path: Vec<u8>,
    method: Vec<u8>,
}

pub struct Handler {
    pub packet: PacketTarget,
    pub selector: Selector,
    pub action: Action,
    pub sender: Sender<RequestInfo>,
    pub receiver: Receiver<RequestInfo>,
}

impl Handler {
    pub fn new(config:Config, sender: Sender<RequestInfo>, receiver: Receiver<RequestInfo>) -> Handler {
        Handler {
            packet: config.packet,
            selector: config.selector,
            action: config.action,
            sender: sender,
            receiver: receiver
        }
    }

    pub fn handle_http_message(&self, message: &HttpMessage) -> Option<Action> {
        return match &message.start_line {
            StartLine::Request(request_line) => match self.packet {
                PacketTarget::Request => {
                    if select_request(request_line, &message.header_fields, &self.selector) {
                        Some(self.action.clone())
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
            },
            StartLine::Status(status_line) => match self.packet {
                PacketTarget::Request => {
                    None
                },
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
                                Some(self.action.clone())
                            } else {
                                None
                            }
                        }
                        Err(e) => None,
                    }
                }
            },
        }
    }
    

    pub fn handle_http<'a>(
        &self,
        i: &'a[u8],
        body_state: BodyState,
    ) -> Result<(
        &'a[u8],
        &'a[u8],
        &'a[u8],
        BodyState,
        Option<Action>,
    ),io::Error> {
        match body_state {
            BodyState::Complete => match http_state(i) {
                Ok(((header, body, rest), HttpState::Complete(http_message))) => {
                    print!("parse Complete ");
                    match http_message.start_line {
                        StartLine::Request(_) => {println!("Request")}
                        StartLine::Status(_) => {println!("Response")}
                    };
                    return Ok((
                        header,
                        body,
                        rest,
                        BodyState::Complete,
                        self.handle_http_message(&http_message),
                    ))
                }
                Ok(((header, body, rest), HttpState::Incomplete(http_message))) => {
                    print!("parse Incomplete {:?}",http_message.start_line);
                    match http_message.start_line {
                        StartLine::Request(_) => {println!("Request")}
                        StartLine::Status(_) => {println!("Response")}
                    };
                    return Ok((
                        header,
                        body,
                        rest,
                        BodyState::Complete,
                        self.handle_http_message(&http_message),
                    ))
                }
                Err(e) => return match e {
                    Incomplete(_) => {
                        println!("parseErr ");
                        Err(io::Error::new(io::ErrorKind::InvalidData, " invalid data: Incomplete "))
                    },
                    _ => {
                        println!("parseErr ");
                        Err(io::Error::new(io::ErrorKind::InvalidData, " invalid data"))
                    },
                },
            },
            _ => match body(i, body_state) {
                Ok((o, BodyState::Complete)) => {
                    return Ok((&i[..0], i, o, BodyState::Complete, None))
                }
                Ok((o, state)) => return Ok((&i[..0], i, o, state, None)),
                Err(e) => return match e {
                    Incomplete(_) => {
                        Err(io::Error::new(io::ErrorKind::InvalidData, " invalid data: Incomplete "))
                    },
                    _ => {
                        Err(io::Error::new(io::ErrorKind::InvalidData, " invalid data"))
                    },
                },
            },
        };
    }

    pub async fn handle_stream<INPUT: AsyncReadExt + Unpin, OUTPUT: AsyncWriteExt + Unpin>(
        &self,
        mut reader: INPUT,
        mut writer: OUTPUT,
    ) -> io::Result<()> {
        let mut body_state = BodyState::Complete;
        loop {
            let mut buf_in = [0u8; 4 * 1024usize];
            let n = match reader.read(&mut buf_in).await {
                // socket closed
                Ok(n) if n == 0 => return Ok(()),
                Ok(n) => n,
                Err(e) => {
                    eprintln!("failed to read from socket; err = {:?}", e);
                    return Ok(());
                }
            };
            let buf_slice = &mut buf_in[..n];
            loop {
                match self.handle_http(buf_slice, body_state) {
                    Ok((header, body, rest, state, action)) => {
                        println!("take action {:?}",action);
                        match action {
                            Some(action) => match action {
                                Action::Abort => {
                                    return Ok(());
                                }
                                Action::Delay(duration) => {
                                    sleep(duration.to_owned()).await;
                                    if let Err(e) = writer.write_all(header).await {
                                        eprintln!("failed to write to socket; err = {:?}", e);
                                        return Ok(());
                                    }
                                    if let Err(e) = writer.write_all(body).await {
                                        eprintln!("failed to write to socket; err = {:?}", e);
                                        return Ok(());
                                    }
                                }
                                Action::Replace(http_message) => {
                                    if let Err(e) = writer.write_all(http_message.as_slice()).await {
                                        eprintln!("failed to write to socket; err = {:?}", e);
                                        return Ok(());
                                    }
                                    if let Err(e) = writer.write_all(body).await {
                                        eprintln!("failed to write to socket; err = {:?}", e);
                                        return Ok(());
                                    }
                                }
                            },
                            None => {
                                if let Err(e) = writer.write_all(header).await {
                                    eprintln!("failed to write to socket; err = {:?}", e);
                                    return Ok(());
                                }
                                if let Err(e) = writer.write_all(body).await {
                                    eprintln!("failed to write to socket; err = {:?}", e);
                                    return Ok(());
                                }
                            }
                        };
                        body_state = state;
                        if rest.is_empty() {
                            break;
                        }
                    }
                    Err(e) => {
                        if let Err(e) = writer.write_all(&buf_slice).await {
                            eprintln!("failed to write to socket; err = {:?}", e);
                            return Ok(());
                        }
                        body_state = BodyState::Complete;
                        break;
                    }
                };
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct Config {
    pub packet: PacketTarget,
    pub selector: Selector,
    pub action: Action,
}



#[test]
fn testtt() {
    let header_fields = vec![1, 2, 3];
    let header_fields0 = vec![2, 4, 5];
    assert_eq!(
        header_fields
            .iter()
            .any(|x| header_fields0.iter().any(|y| y == x)),
        true
    );
}

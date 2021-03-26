use std::collections::HashMap;
use std::time::Duration;

use http::{Request, Response};
use hyper::Body;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Clone, Copy, Deserialize, Serialize)]
pub enum PacketTarget {
    Request,
    Response,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct Selector {
    pub path: Option<String>,
    pub method: Option<String>,
    pub code: Option<u16>,
    pub header_fields: Option<HashMap<String, String>>,
}

pub fn select_request(request: &Request<Body>, selector: &Selector) -> bool {
    selector
        .path
        .as_ref()
        .into_iter()
        .all(|p| request.uri().path().starts_with(p))
        && selector
            .method
            .as_ref()
            .into_iter()
            .all(|m| request.method().as_str() == m.to_uppercase())
        && selector.header_fields.as_ref().into_iter().all(|fields| {
            fields.iter().all(|(header, value)| {
                request
                    .headers()
                    .get_all(header)
                    .iter()
                    .any(|f| f.as_bytes() == value.as_bytes())
            })
        })
}

pub fn select_response(
    request: &Request<Body>,
    response: &Response<Body>,
    selector: &Selector,
) -> bool {
    selector
        .path
        .as_ref()
        .into_iter()
        .all(|p| request.uri().path().starts_with(p))
        && selector
            .method
            .as_ref()
            .into_iter()
            .all(|m| request.method().as_str() == m.to_uppercase())
        && selector
            .code
            .as_ref()
            .into_iter()
            .all(|code| response.status().as_u16() == *code)
        && selector.header_fields.as_ref().into_iter().all(|fields| {
            fields.iter().all(|(header, value)| {
                response
                    .headers()
                    .get_all(header)
                    .iter()
                    .any(|f| f.as_bytes() == value.as_bytes())
            })
        })
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub enum Action {
    Replace(Vec<u8>),
    Delay(Duration),
    Abort,
}

// #[derive(Debug, Eq, PartialEq)]
// pub struct RequestInfo {
//     path: Vec<u8>,
//     method: Vec<u8>,
// }

// pub struct Handler {
//     pub packet: PacketTarget,
//     pub selector: Selector,
//     pub action: Action,
//     pub sender: Sender<RequestInfo>,
//     pub receiver: Receiver<RequestInfo>,
// }

// impl Handler {
//     pub fn new(
//         config: Config,
//         sender: Sender<RequestInfo>,
//         receiver: Receiver<RequestInfo>,
//     ) -> Handler {
//         Handler {
//             packet: config.packet,
//             selector: config.selector,
//             action: config.action,
//             sender: sender,
//             receiver: receiver,
//         }
//     }

//     pub fn handle_http_message(&self, message: &HttpMessage) -> Option<Action> {
//         return match &message.start_line {
//             StartLine::Request(request_line) => match self.packet {
//                 PacketTarget::Request => {
//                     if select_request(request_line, &message.header_fields, &self.selector) {
//                         Some(self.action.clone())
//                     } else {
//                         None
//                     }
//                 }
//                 PacketTarget::Response => {
//                     let _ = self.sender.send(RequestInfo {
//                         path: request_line.path.to_vec(),
//                         method: request_line.method.to_vec(),
//                     });
//                     None
//                 }
//             },
//             StartLine::Status(status_line) => match self.packet {
//                 PacketTarget::Request => None,
//                 PacketTarget::Response => match self.receiver.try_recv() {
//                     Ok(request_info) => {
//                         if select_response(
//                             request_info.path.as_slice(),
//                             request_info.method.as_slice(),
//                             status_line.code,
//                             &message.header_fields,
//                             &self.selector,
//                         ) {
//                             Some(self.action.clone())
//                         } else {
//                             None
//                         }
//                     }
//                     Err(_) => None,
//                 },
//             },
//         };
//     }

//     pub fn handle_http<'a>(
//         &self,
//         i: &'a [u8],
//         body_state: BodyState,
//     ) -> Result<(&'a [u8], &'a [u8], &'a [u8], BodyState, Option<Action>), io::Error> {
//         match body_state {
//             BodyState::Complete => match http_state(i) {
//                 Ok(((header, body, rest), HttpState::Complete(http_message))) => {
//                     print!("parse Complete ");
//                     match http_message.start_line {
//                         StartLine::Request(_) => println!("Request"),
//                         StartLine::Status(_) => println!("Response"),
//                     };
//                     return Ok((
//                         header,
//                         body,
//                         rest,
//                         BodyState::Complete,
//                         self.handle_http_message(&http_message),
//                     ));
//                 }
//                 Ok(((header, body, rest), HttpState::Incomplete(http_message))) => {
//                     print!("parse Incomplete {:?}", http_message.start_line);
//                     match http_message.start_line {
//                         StartLine::Request(_) => println!("Request"),
//                         StartLine::Status(_) => println!("Response"),
//                     };
//                     return Ok((
//                         header,
//                         body,
//                         rest,
//                         BodyState::Complete,
//                         self.handle_http_message(&http_message),
//                     ));
//                 }
//                 Err(e) => {
//                     return match e {
//                         Incomplete(_) => {
//                             println!("parseErr ");
//                             Err(io::Error::new(
//                                 io::ErrorKind::InvalidData,
//                                 " invalid data: Incomplete ",
//                             ))
//                         }
//                         _ => {
//                             println!("parseErr ");
//                             Err(io::Error::new(io::ErrorKind::InvalidData, " invalid data"))
//                         }
//                     }
//                 }
//             },
//             _ => match body(i, body_state) {
//                 Ok((o, BodyState::Complete)) => {
//                     return Ok((&i[..0], i, o, BodyState::Complete, None))
//                 }
//                 Ok((o, state)) => return Ok((&i[..0], i, o, state, None)),
//                 Err(e) => {
//                     return match e {
//                         Incomplete(_) => Err(io::Error::new(
//                             io::ErrorKind::InvalidData,
//                             " invalid data: Incomplete ",
//                         )),
//                         _ => Err(io::Error::new(io::ErrorKind::InvalidData, " invalid data")),
//                     }
//                 }
//             },
//         };
//     }

//     pub async fn handle_stream<INPUT: AsyncReadExt + Unpin, OUTPUT: AsyncWriteExt + Unpin>(
//         &self,
//         mut reader: INPUT,
//         mut writer: OUTPUT,
//     ) -> io::Result<()> {
//         let mut body_state = BodyState::Complete;
//         loop {
//             let mut buf_in = [0u8; 4 * 1024usize];
//             let n = match reader.read(&mut buf_in).await {
//                 // socket closed
//                 Ok(n) if n == 0 => return Ok(()),
//                 Ok(n) => n,
//                 Err(e) => {
//                     eprintln!("failed to read from socket; err = {:?}", e);
//                     return Ok(());
//                 }
//             };
//             let buf_slice = &mut buf_in[..n];
//             loop {
//                 match self.handle_http(buf_slice, body_state) {
//                     Ok((header, body, rest, state, action)) => {
//                         println!("take action {:?}", action);
//                         match action {
//                             Some(action) => match action {
//                                 Action::Abort => {
//                                     return Ok(());
//                                 }
//                                 Action::Delay(duration) => {
//                                     sleep(duration.to_owned()).await;
//                                     if let Err(e) = writer.write_all(header).await {
//                                         eprintln!("failed to write to socket; err = {:?}", e);
//                                         return Ok(());
//                                     }
//                                     if let Err(e) = writer.write_all(body).await {
//                                         eprintln!("failed to write to socket; err = {:?}", e);
//                                         return Ok(());
//                                     }
//                                 }
//                                 Action::Replace(http_message) => {
//                                     if let Err(e) = writer.write_all(http_message.as_slice()).await
//                                     {
//                                         eprintln!("failed to write to socket; err = {:?}", e);
//                                         return Ok(());
//                                     }
//                                     if let Err(e) = writer.write_all(body).await {
//                                         eprintln!("failed to write to socket; err = {:?}", e);
//                                         return Ok(());
//                                     }
//                                 }
//                             },
//                             None => {
//                                 if let Err(e) = writer.write_all(header).await {
//                                     eprintln!("failed to write to socket; err = {:?}", e);
//                                     return Ok(());
//                                 }
//                                 if let Err(e) = writer.write_all(body).await {
//                                     eprintln!("failed to write to socket; err = {:?}", e);
//                                     return Ok(());
//                                 }
//                             }
//                         };
//                         body_state = state;
//                         if rest.is_empty() {
//                             break;
//                         }
//                     }
//                     Err(_) => {
//                         if let Err(e) = writer.write_all(&buf_slice).await {
//                             eprintln!("failed to write to socket; err = {:?}", e);
//                             return Ok(());
//                         }
//                         body_state = BodyState::Complete;
//                         break;
//                     }
//                 };
//             }
//         }
//     }
// }

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct Config {
    pub action: Action,
    pub packet: PacketTarget,
    pub selector: Selector,
}

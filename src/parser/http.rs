use super::util::take_http;
use httparse::*;
use httparse::Status;
use nom::AsChar;

#[derive(Debug, Eq, PartialEq)]
pub enum HttpPacket<'headers,'buf: 'headers> {
    Request(httparse::Request<'headers,'buf>),
    Response(httparse::Response<'headers,'buf>)
}

impl<'h, 'b> HttpPacket<'h, 'b> {
    pub fn new(headers: &'h mut [httparse::Header<'b>], buf: &'b [u8]) -> HttpPacket<'h,'b> {
        match take_http(buf) {
            Ok(_) => {
                let rsp = httparse::Response::new(headers.as_mut());
                HttpPacket::Response(rsp)
            },
            Err(_) => {
                let req = httparse::Request::new(headers.as_mut());
                HttpPacket::Request(req)
            },
        }
    }

    pub fn parse_once(&mut self, buf: &'b [u8]) -> Result<usize> {
        match self {
            HttpPacket::Request(req) => {
                req.parse(buf)
            }
            HttpPacket::Response(rsp) => {
                rsp.parse(buf)
            }
        }
    }

    pub fn parse_first_line(&mut self, buf:&'b [u8]) -> Result<usize> {
        let result:Result<usize> = self.parse_once(buf);
        match result {
            Ok(status) => {
                if status.is_complete() {
                    return Ok(Status::Complete(status.unwrap()));
                }
            }
            Err(Error::TooManyHeaders) => {
                return Ok(Status::Complete(HttpPacket::get_first_line_length(self)));
            }
            Err(e) => {
                return Err(e);
            }
        };

        match self {
            HttpPacket::Request(req) => {
                if req.method.is_some()&&req.path.is_some()&&req.version.is_some() {
                    Ok(Status::Complete(HttpPacket::get_request_first_line_length(req)))
                }else {
                    Ok(Status::Partial)
                }
            }
            HttpPacket::Response(rsp) => {
                if rsp.version.is_some()&&rsp.reason.is_some()&&rsp.code.is_some() {
                    Ok(Status::Complete(HttpPacket::get_response_first_line_length(rsp)))
                }else {
                    Ok(Status::Partial)
                }
            }
        }
    }

    fn get_first_line_length(packet:&HttpPacket) ->usize {
        match packet {
            HttpPacket::Request(req) => {
                HttpPacket::get_request_first_line_length(req)
            }
            HttpPacket::Response(rsp) => {
                HttpPacket::get_response_first_line_length(rsp)
            }
        }
    }

    fn get_request_first_line_length(req: &httparse::Request) -> usize {
        req.method.unwrap().len() + 1 + req.path.unwrap().len() + 1 + req.version.unwrap().len() + 7
    }

    fn get_response_first_line_length(rsp:&httparse::Response) -> usize {
        rsp.version.unwrap().len() + 7 + 1 + 3 + rsp.reason.unwrap().len()
    }
}

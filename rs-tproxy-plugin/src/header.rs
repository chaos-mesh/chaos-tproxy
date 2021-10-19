use std::collections::HashMap;

use http::{request, response, Request, Response, Version};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestHeader<'a> {
    pub method: String,
    pub uri: String,
    pub version: String,

    #[serde(borrow)]
    pub header_map: HashMap<&'a str, Vec<Vec<u8>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseHeader<'a> {
    pub status_code: u16,
    pub version: String,

    #[serde(borrow)]
    pub header_map: HashMap<&'a str, Vec<Vec<u8>>>,
}

impl RequestHeader<'_> {
    pub fn build<T>(&self, body: T) -> anyhow::Result<Request<T>> {
        let mut req_builder = Request::builder()
            .method(self.method.as_str())
            .uri(self.uri.as_str())
            .version(parse_version(&self.version)?);
        for (k, list) in self.header_map.iter() {
            for v in list {
                req_builder = req_builder.header(*k, v.as_slice())
            }
        }
        Ok(req_builder.body(body)?)
    }
}

impl ResponseHeader<'_> {
    pub fn build<T>(&self, body: T) -> anyhow::Result<Response<T>> {
        let mut resp_builder = Response::builder()
            .status(self.status_code)
            .version(parse_version(&self.version)?);
        for (k, list) in self.header_map.iter() {
            for v in list {
                resp_builder = resp_builder.header(*k, v.as_slice())
            }
        }
        Ok(resp_builder.body(body)?)
    }
}

impl<'a> From<&'a request::Parts> for RequestHeader<'a> {
    fn from(parts: &'a request::Parts) -> Self {
        Self {
            method: parts.method.to_string(),
            uri: parts.uri.to_string(),
            version: format!("{:?}", parts.version),
            header_map: make_header_map(&parts.headers),
        }
    }
}

impl<'a> From<&'a response::Parts> for ResponseHeader<'a> {
    fn from(parts: &'a response::Parts) -> Self {
        Self {
            status_code: parts.status.as_u16(),
            version: format!("{:?}", parts.version),
            header_map: make_header_map(&parts.headers),
        }
    }
}

fn make_header_map(raw: &http::HeaderMap<http::HeaderValue>) -> HashMap<&'_ str, Vec<Vec<u8>>> {
    let mut map = HashMap::<&str, Vec<Vec<u8>>>::new();
    for (name, value) in raw.into_iter() {
        let key = name.as_str();
        match map.get_mut(key) {
            Some(v) => v.push(value.as_bytes().to_owned()),
            None => {
                map.insert(key, vec![value.as_bytes().to_owned()]);
            }
        }
    }
    map
}

fn parse_version(version: &str) -> anyhow::Result<Version> {
    match version {
        "HTTP/0.9" => Ok(Version::HTTP_09),
        "HTTP/1.0" => Ok(Version::HTTP_10),
        "HTTP/1.1" => Ok(Version::HTTP_11),
        "HTTP/2.0" => Ok(Version::HTTP_2),
        "HTTP/3.0" => Ok(Version::HTTP_3),
        _ => Err(anyhow::anyhow!("unsupported http version: {}", version)),
    }
}

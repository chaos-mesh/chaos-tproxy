use std::collections::HashMap;
use std::convert::TryInto;
use std::time::Duration;

use anyhow::anyhow;
use http::header::HeaderName;
use http::{Method, Request, Response, Uri};
use hyper::Body;
use serde_derive::{Deserialize, Serialize};
use tokio::time::sleep;
use tracing::{debug, instrument};
use url::Url;

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
    uri: Uri,
    method: Method,
    response: &Response<Body>,
    selector: &Selector,
) -> bool {
    selector
        .path
        .as_ref()
        .into_iter()
        .all(|p| uri.path().starts_with(p))
        && selector
            .method
            .as_ref()
            .into_iter()
            .all(|m| method.as_str() == m.to_uppercase())
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
    Abort,
    Delay(Duration),
    Append {
        queries: Option<Vec<(String, String)>>,
        headers: Option<Vec<(String, String)>>,
    },
    Replace {
        path: Option<String>,
        method: Option<String>,
        code: Option<u16>,
        body: Option<Vec<u8>>,
        queries: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
    },
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct Config {
    pub action: Action,
    pub packet: PacketTarget,
    pub selector: Selector,
}

// TODO: preprocess config to avoid unnecessary error in parsing
#[instrument]
pub async fn apply_request_action(
    mut request: Request<Body>,
    action: &Action,
) -> anyhow::Result<Request<Body>> {
    match action {
        Action::Abort => return Err(anyhow!("Abort applied")),
        Action::Delay(dur) => sleep(*dur).await,
        Action::Append { queries, headers } => {
            if let Some(qs) = &queries {
                let mut url: Url = request.uri().to_string().parse()?;
                url.query_pairs_mut().extend_pairs(qs);
                *request.uri_mut() = url.to_string().parse()?;
            }

            if let Some(hdrs) = &headers {
                for (key, value) in hdrs {
                    request
                        .headers_mut()
                        .append(key.parse::<HeaderName>()?, value.parse()?);
                }
            }
        }
        Action::Replace {
            path,
            method,
            code: _,
            body,
            queries,
            headers,
        } => {
            if let Some(p) = &path {
                let mut url: Url = request.uri().to_string().parse()?;
                url.set_path(p);
                *request.uri_mut() = url.to_string().parse()?;
            }

            if let Some(md) = method {
                *request.method_mut() = md.parse()?;
            }

            if let Some(data) = body {
                *request.body_mut() = data.clone().into()
            }

            if let Some(qs) = &queries {
                let url: Url = request.uri().to_string().parse()?;

                let mut new_url = url.clone();
                let mut query_part = new_url.query_pairs_mut();
                query_part.clear();

                for (key, value) in url.query_pairs() {
                    if let Some(v) = qs.get(&*key) {
                        query_part.append_pair(&key, v);
                    } else {
                        query_part.append_pair(&key, &value);
                    }
                }

                *request.uri_mut() = url.to_string().parse()?;
            }

            if let Some(hdrs) = &headers {
                for (key, value) in hdrs {
                    request
                        .headers_mut()
                        .insert(key.parse::<HeaderName>()?, value.parse()?);
                }
            }
        }
    }

    debug!("action applied: {:?}", request);
    Ok(request)
}

// TODO: preprocess config to avoid unnecessary error in parsing
#[instrument]
pub async fn apply_response_action(
    mut response: Response<Body>,
    action: &Action,
) -> anyhow::Result<Response<Body>> {
    match action {
        Action::Abort => return Err(anyhow!("Abort applied")),
        Action::Delay(dur) => sleep(*dur).await,
        Action::Append {
            queries: _,
            headers,
        } => {
            if let Some(hdrs) = &headers {
                for (key, value) in hdrs {
                    response
                        .headers_mut()
                        .append(key.parse::<HeaderName>()?, value.parse()?);
                }
            }
        }
        Action::Replace {
            path: _,
            method: _,
            code,
            body,
            queries: _,
            headers,
        } => {
            if let Some(co) = code {
                *response.status_mut() = (*co).try_into()?;
            }

            if let Some(data) = body {
                *response.body_mut() = data.clone().into()
            }

            if let Some(hdrs) = &headers {
                for (key, value) in hdrs {
                    response
                        .headers_mut()
                        .insert(key.parse::<HeaderName>()?, value.parse()?);
                }
            }
        }
    }

    debug!("action applied: {:?}", response);
    Ok(response)
}
